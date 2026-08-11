mod remote;
mod repository;

#[cfg(test)]
mod tests;

pub(crate) use remote::JmFavoriteRemote;

use self::{remote::FavoriteRemote, repository::FavoriteSyncRepository};
use crate::{
    application::{account::AccountSession, FavoriteInput, FavoriteItem, FavoriteService},
    http_error::HttpError,
    jm::{FavoriteComic, FavoriteOrder},
};
use axum::http::StatusCode;
use chrono::Utc;
use repository::SyncRecord;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;
use std::{collections::HashSet, sync::Arc};
use tokio::sync::{watch, Mutex};

const REMOTE_PAGE_SIZE: usize = 20;
const MAX_REMOTE_PAGES: u32 = 10_000;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum FavoriteSyncStatus {
    Disabled,
    Checking,
    NeedsResolution,
    Syncing,
    Synced,
    Error,
}

impl FavoriteSyncStatus {
    fn db_value(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::Checking => "checking",
            Self::NeedsResolution => "needs_resolution",
            Self::Syncing => "syncing",
            Self::Synced => "synced",
            Self::Error => "error",
        }
    }

    fn from_db(value: String) -> anyhow::Result<Self> {
        match value.as_str() {
            "disabled" => Ok(Self::Disabled),
            "checking" => Ok(Self::Checking),
            "needs_resolution" => Ok(Self::NeedsResolution),
            "syncing" => Ok(Self::Syncing),
            "synced" => Ok(Self::Synced),
            "error" => Ok(Self::Error),
            _ => anyhow::bail!("unknown favorite sync status: {value}"),
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum FavoriteSyncProgressPhase {
    FetchingRemote,
    UploadingLocal,
    Verifying,
}

impl FavoriteSyncProgressPhase {
    fn db_value(self) -> &'static str {
        match self {
            Self::FetchingRemote => "fetching_remote",
            Self::UploadingLocal => "uploading_local",
            Self::Verifying => "verifying",
        }
    }

    fn from_db(value: &str) -> anyhow::Result<Self> {
        match value {
            "fetching_remote" => Ok(Self::FetchingRemote),
            "uploading_local" => Ok(Self::UploadingLocal),
            "verifying" => Ok(Self::Verifying),
            _ => anyhow::bail!("unknown favorite sync progress phase: {value}"),
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum PendingKind {
    Check,
    Merge,
    RemoteOverwrite,
    SetFavorite,
}

impl PendingKind {
    fn db_value(self) -> &'static str {
        match self {
            Self::Check => "check",
            Self::Merge => "merge",
            Self::RemoteOverwrite => "remote_overwrite",
            Self::SetFavorite => "set_favorite",
        }
    }

    fn from_db(value: &str) -> anyhow::Result<Self> {
        match value {
            "check" => Ok(Self::Check),
            "merge" => Ok(Self::Merge),
            "remote_overwrite" => Ok(Self::RemoteOverwrite),
            "set_favorite" => Ok(Self::SetFavorite),
            _ => anyhow::bail!("unknown pending favorite sync operation: {value}"),
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum FavoriteSyncResolution {
    Merge,
    RemoteOverwrite,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FavoriteSyncState {
    pub enabled: bool,
    pub status: FavoriteSyncStatus,
    pub account_username: Option<String>,
    pub local_count: i64,
    pub remote_count: i64,
    pub local_only_count: i64,
    pub remote_only_count: i64,
    pub progress_done: i64,
    pub progress_total: i64,
    pub progress_phase: Option<FavoriteSyncProgressPhase>,
    pub pending_kind: Option<PendingKind>,
    pub pending_comic_id: Option<String>,
    pub pending_target: Option<bool>,
    pub last_error: Option<String>,
    pub last_checked_at: Option<i64>,
    pub last_synced_at: Option<i64>,
}

impl From<&SyncRecord> for FavoriteSyncState {
    fn from(record: &SyncRecord) -> Self {
        Self {
            enabled: record.enabled,
            status: record.status,
            account_username: record.account_username.clone(),
            local_count: record.local_count,
            remote_count: record.remote_count,
            local_only_count: record.local_only_count,
            remote_only_count: record.remote_only_count,
            progress_done: record.progress_done,
            progress_total: record.progress_total,
            progress_phase: record.progress_phase,
            pending_kind: record.pending_kind,
            pending_comic_id: record.pending_comic_id.clone(),
            pending_target: record.pending_target,
            last_error: record.last_error.clone(),
            last_checked_at: record.last_checked_at,
            last_synced_at: record.last_synced_at,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum FavoriteSyncError {
    #[error("{0}")]
    Conflict(String),
    #[error("{0}")]
    Account(String),
    #[error("{0}")]
    OrderUnavailable(String),
    #[error("{0}")]
    SyncMismatch(String),
    #[error("{0}")]
    OrderStale(String),
    #[error("{0}")]
    Remote(String),
    #[error("{0}")]
    Internal(String),
}

impl FavoriteSyncError {
    pub fn into_http_error(self) -> HttpError {
        match self {
            Self::Conflict(message) | Self::Account(message) => {
                HttpError::new(StatusCode::CONFLICT, message, false)
            }
            Self::OrderUnavailable(message) => HttpError::new(StatusCode::CONFLICT, message, false)
                .with_code("favorite_order_unavailable"),
            Self::SyncMismatch(message) => HttpError::new(StatusCode::CONFLICT, message, false)
                .with_code("favorite_sync_mismatch"),
            Self::OrderStale(message) => HttpError::new(StatusCode::CONFLICT, message, true)
                .with_code("favorite_order_stale"),
            Self::Remote(message) => HttpError::new(StatusCode::BAD_GATEWAY, message, true),
            Self::Internal(message) => HttpError::internal(message),
        }
    }
}

pub struct FavoriteSyncService {
    favorites: Arc<FavoriteService>,
    remote: Arc<dyn FavoriteRemote>,
    account_rx: watch::Receiver<AccountSession>,
    repository: FavoriteSyncRepository,
    state: Mutex<SyncRecord>,
    operation: Mutex<()>,
}

impl FavoriteSyncService {
    pub(crate) async fn new(
        db: SqlitePool,
        favorites: Arc<FavoriteService>,
        remote: Arc<dyn FavoriteRemote>,
        account_rx: watch::Receiver<AccountSession>,
    ) -> anyhow::Result<Self> {
        let repository = FavoriteSyncRepository::new(db);
        let mut state = repository.load().await?;
        let mut changed = false;

        if !state.enabled && state.status != FavoriteSyncStatus::Disabled {
            reset_disabled(&mut state);
            changed = true;
        } else if state.enabled
            && matches!(
                state.status,
                FavoriteSyncStatus::Checking | FavoriteSyncStatus::Syncing
            )
        {
            state.status = FavoriteSyncStatus::Error;
            state.last_error = Some("上次收藏同步被服务重启中断，请重试同步".into());
            state.progress_done = 0;
            state.progress_total = 0;
            state.progress_phase = None;
            state.updated_at = now();
            changed = true;
        }

        if changed {
            repository.save(&state).await?;
        }

        Ok(Self {
            favorites,
            remote,
            account_rx,
            repository,
            state: Mutex::new(state),
            operation: Mutex::new(()),
        })
    }

    pub fn start_account_monitor(self: &Arc<Self>) {
        let service = self.clone();
        let mut account_rx = self.account_rx.clone();
        tokio::spawn(async move {
            let initial = account_rx.borrow().clone();
            service.handle_account_session(initial).await;
            while account_rx.changed().await.is_ok() {
                let session = account_rx.borrow().clone();
                service.handle_account_session(session).await;
            }
        });
    }

    pub async fn state(&self) -> FavoriteSyncState {
        FavoriteSyncState::from(&*self.state.lock().await)
    }

    pub(crate) async fn list(
        &self,
        page: u32,
        page_size: u32,
        order: FavoriteOrder,
    ) -> Result<(Vec<FavoriteItem>, i64), FavoriteSyncError> {
        let mut account_rx = self.account_rx.clone();
        let session = account_rx.borrow_and_update().clone();
        let remote_epoch = {
            let state = self.state.lock().await;
            remote_order_ready(&state, &session).then_some(state.operation_epoch)
        };

        let Some(epoch) = remote_epoch else {
            if order == FavoriteOrder::Mp {
                return Err(FavoriteSyncError::OrderUnavailable(
                    "登录 JM 账号并完成收藏同步后才能按更新时间排序".into(),
                ));
            }
            return self
                .favorites
                .list(page, page_size)
                .await
                .map_err(|error| local_operation_error("读取本地收藏", error));
        };

        let remote_page = self
            .remote
            .favorite_page(page, order)
            .await
            .map_err(|error| {
                tracing::error!(%error, ?order, page, "failed to read ordered remote favorites");
                FavoriteSyncError::Remote("读取远端收藏失败，请重试".into())
            })?;

        // Network reads must not block synchronized writes. Once the response arrives,
        // briefly serialize local assembly so count and item reads describe one state.
        let _operation = self.operation.lock().await;
        self.validate_order_context(epoch, &session, &mut account_rx)
            .await?;

        let local_total = self
            .favorites
            .count()
            .await
            .map_err(|error| local_operation_error("读取本地收藏数量", error))?;
        if local_total != i64::from(remote_page.total) {
            return Err(FavoriteSyncError::SyncMismatch(
                "本地与远端收藏已不一致，请在设置中重新检查同步".into(),
            ));
        }

        let mut seen = HashSet::with_capacity(remote_page.items.len());
        let mut comic_ids = Vec::with_capacity(remote_page.items.len());
        for item in remote_page.items {
            let comic_id = item.id.trim().to_string();
            if comic_id.is_empty()
                || !comic_id.chars().all(|character| character.is_ascii_digit())
                || !seen.insert(comic_id.clone())
            {
                return Err(FavoriteSyncError::SyncMismatch(
                    "远端收藏列表异常，请在设置中重新检查同步".into(),
                ));
            }
            comic_ids.push(comic_id);
        }

        let items = self
            .favorites
            .by_ids_in_order(&comic_ids)
            .await
            .map_err(|error| local_operation_error("按远端顺序读取本地收藏", error))?;
        if items.len() != comic_ids.len() {
            return Err(FavoriteSyncError::SyncMismatch(
                "本地与远端收藏已不一致，请在设置中重新检查同步".into(),
            ));
        }
        self.validate_order_context(epoch, &session, &mut account_rx)
            .await?;
        Ok((items, local_total))
    }

    pub async fn set_enabled(
        self: &Arc<Self>,
        enabled: bool,
    ) -> Result<FavoriteSyncState, FavoriteSyncError> {
        if !enabled {
            self.disable().await?;
            return Ok(self.state().await);
        }

        let session = self.logged_in_session()?;
        let epoch = {
            let mut state = self.state.lock().await;
            if state.enabled {
                return Ok(FavoriteSyncState::from(&*state));
            }
            let mut next = state.clone();
            next.enabled = true;
            next.account_id = session.user_id;
            next.account_username = session.username;
            next.status = FavoriteSyncStatus::Checking;
            next.operation_epoch = next.operation_epoch.saturating_add(1);
            set_pending(&mut next, PendingKind::Check, None, None, None);
            clear_counts_and_error(&mut next);
            next.updated_at = now();
            self.save_locked(&mut state, next)
                .await
                .map_err(internal_error)?
                .operation_epoch
        };
        self.spawn_check(epoch);
        Ok(self.state().await)
    }

    pub async fn check(self: &Arc<Self>) -> Result<FavoriteSyncState, FavoriteSyncError> {
        let epoch = self.begin_check(false).await?;
        self.spawn_check(epoch);
        Ok(self.state().await)
    }

    pub async fn resolve(
        self: &Arc<Self>,
        resolution: FavoriteSyncResolution,
    ) -> Result<FavoriteSyncState, FavoriteSyncError> {
        let session = self.logged_in_session()?;
        let (epoch, kind) = {
            let mut state = self.state.lock().await;
            if !state.enabled || state.status != FavoriteSyncStatus::NeedsResolution {
                return Err(FavoriteSyncError::Conflict(
                    "当前没有需要处理的收藏差异".into(),
                ));
            }
            if state.account_id != session.user_id {
                return Err(FavoriteSyncError::Account(
                    "当前登录账号与收藏同步绑定账号不一致".into(),
                ));
            }
            let kind = match resolution {
                FavoriteSyncResolution::Merge => PendingKind::Merge,
                FavoriteSyncResolution::RemoteOverwrite => PendingKind::RemoteOverwrite,
            };
            let mut next = state.clone();
            next.status = FavoriteSyncStatus::Syncing;
            next.operation_epoch = next.operation_epoch.saturating_add(1);
            set_pending(&mut next, kind, None, None, None);
            next.last_error = None;
            next.progress_done = 0;
            next.progress_total = 0;
            next.progress_phase = None;
            next.updated_at = now();
            let saved = self
                .save_locked(&mut state, next)
                .await
                .map_err(internal_error)?;
            (saved.operation_epoch, kind)
        };
        self.spawn_pending(epoch, kind);
        Ok(self.state().await)
    }

    pub async fn retry(self: &Arc<Self>) -> Result<FavoriteSyncState, FavoriteSyncError> {
        let session = self.logged_in_session()?;
        let (epoch, kind) = {
            let mut state = self.state.lock().await;
            if !state.enabled || state.status != FavoriteSyncStatus::Error {
                return Err(FavoriteSyncError::Conflict(
                    "当前没有可重试的收藏同步任务".into(),
                ));
            }
            if state.account_id != session.user_id {
                return Err(FavoriteSyncError::Account(
                    "当前登录账号与收藏同步绑定账号不一致".into(),
                ));
            }
            let kind = state.pending_kind.ok_or_else(|| {
                FavoriteSyncError::Conflict("收藏同步错误缺少可重试任务，请关闭后重新开启".into())
            })?;
            let mut next = state.clone();
            next.status = if kind == PendingKind::Check {
                FavoriteSyncStatus::Checking
            } else {
                FavoriteSyncStatus::Syncing
            };
            next.operation_epoch = next.operation_epoch.saturating_add(1);
            next.last_error = None;
            next.progress_done = 0;
            next.progress_total = 0;
            next.updated_at = now();
            let saved = self
                .save_locked(&mut state, next)
                .await
                .map_err(internal_error)?;
            (saved.operation_epoch, kind)
        };
        self.spawn_pending(epoch, kind);
        Ok(self.state().await)
    }

    pub async fn upsert(
        &self,
        comic_id: &str,
        input: FavoriteInput,
    ) -> Result<FavoriteItem, FavoriteSyncError> {
        let _operation = self.operation.lock().await;
        let payload = serde_json::to_string(&input)
            .map_err(|error| local_operation_error("序列化本地收藏", error))?;
        let epoch = {
            let mut state = self.state.lock().await;
            if !state.enabled {
                return self
                    .favorites
                    .upsert(comic_id, input)
                    .await
                    .map_err(|error| local_operation_error("写入本地收藏", error));
            }
            validate_synced_session(&state, &self.logged_in_session()?)?;
            let mut next = state.clone();
            next.status = FavoriteSyncStatus::Syncing;
            next.operation_epoch = next.operation_epoch.saturating_add(1);
            set_pending(
                &mut next,
                PendingKind::SetFavorite,
                Some(comic_id.to_string()),
                Some(true),
                Some(payload),
            );
            next.last_error = None;
            next.updated_at = now();
            self.save_locked(&mut state, next)
                .await
                .map_err(internal_error)?
                .operation_epoch
        };

        let result = async {
            self.ensure_remote_target(epoch, comic_id, true).await?;
            self.upsert_local_if_current(epoch, comic_id, input).await
        }
        .await;

        match result {
            Ok(item) => {
                if let Err(error) = self.finish_synced(epoch).await {
                    self.fail_current(epoch, "收藏已写入，但同步状态保存失败", &error)
                        .await;
                    return Err(FavoriteSyncError::Internal("收藏同步状态保存失败".into()));
                }
                Ok(item)
            }
            Err(error) => {
                self.fail_current(epoch, "远端收藏失败，请在设置中重试同步", &error)
                    .await;
                Err(self
                    .operation_error(epoch, "远端收藏失败，请在设置中重试同步")
                    .await)
            }
        }
    }

    pub async fn remove(&self, comic_id: &str) -> Result<(), FavoriteSyncError> {
        let _operation = self.operation.lock().await;
        let epoch = {
            let mut state = self.state.lock().await;
            if !state.enabled {
                return self
                    .favorites
                    .remove(comic_id)
                    .await
                    .map_err(|error| local_operation_error("删除本地收藏", error));
            }
            validate_synced_session(&state, &self.logged_in_session()?)?;
            let mut next = state.clone();
            next.status = FavoriteSyncStatus::Syncing;
            next.operation_epoch = next.operation_epoch.saturating_add(1);
            set_pending(
                &mut next,
                PendingKind::SetFavorite,
                Some(comic_id.to_string()),
                Some(false),
                None,
            );
            next.last_error = None;
            next.updated_at = now();
            self.save_locked(&mut state, next)
                .await
                .map_err(internal_error)?
                .operation_epoch
        };

        let result = async {
            self.ensure_remote_target(epoch, comic_id, false).await?;
            self.remove_local_if_current(epoch, comic_id).await
        }
        .await;

        match result {
            Ok(()) => {
                if let Err(error) = self.finish_synced(epoch).await {
                    self.fail_current(epoch, "取消收藏已完成，但同步状态保存失败", &error)
                        .await;
                    return Err(FavoriteSyncError::Internal("收藏同步状态保存失败".into()));
                }
                Ok(())
            }
            Err(error) => {
                self.fail_current(epoch, "远端取消收藏失败，请在设置中重试同步", &error)
                    .await;
                Err(self
                    .operation_error(epoch, "远端取消收藏失败，请在设置中重试同步")
                    .await)
            }
        }
    }

    pub async fn clear(&self) -> Result<(), FavoriteSyncError> {
        let _operation = self.operation.lock().await;
        let state = self.state.lock().await;
        if state.enabled {
            return Err(FavoriteSyncError::Conflict(
                "收藏同步开启时不能批量清空收藏".into(),
            ));
        }
        self.favorites
            .clear()
            .await
            .map_err(|error| local_operation_error("清空本地收藏", error))
    }

    async fn handle_account_session(self: &Arc<Self>, session: AccountSession) {
        let record = self.state.lock().await.clone();
        if !record.enabled {
            return;
        }

        if session.username.is_none() {
            if let Err(error) = self.disable().await {
                tracing::error!(%error, "failed to disable favorite sync after account removal");
            }
            return;
        }

        if session.login_status == super::account::LoginStatus::LoggingIn {
            if matches!(
                record.status,
                FavoriteSyncStatus::Checking | FavoriteSyncStatus::Syncing
            ) {
                self.mark_account_unavailable("JM 账号正在重新登录，请登录成功后重试同步")
                    .await;
            }
            return;
        }

        if session.login_status == super::account::LoginStatus::LoggedOut {
            self.mark_account_unavailable("JM 账号未登录，请重新登录后重试同步")
                .await;
            return;
        }

        if record.account_id != session.user_id {
            if let Err(error) = self.disable().await {
                tracing::error!(%error, "failed to disable favorite sync after account switch");
            }
            return;
        }

        if matches!(
            record.status,
            FavoriteSyncStatus::Synced | FavoriteSyncStatus::NeedsResolution
        ) {
            match self.begin_check(true).await {
                Ok(epoch) => self.spawn_check(epoch),
                Err(FavoriteSyncError::Conflict(_)) => {}
                Err(error) => tracing::error!(%error, "failed to start favorite sync login check"),
            }
        }
    }

    async fn disable(&self) -> Result<(), FavoriteSyncError> {
        let mut state = self.state.lock().await;
        let mut next = state.clone();
        next.operation_epoch = next.operation_epoch.saturating_add(1);
        reset_disabled(&mut next);
        next.updated_at = now();
        self.save_locked(&mut state, next)
            .await
            .map(|_| ())
            .map_err(internal_error)
    }

    async fn mark_account_unavailable(&self, message: &str) {
        let mut state = self.state.lock().await;
        if !state.enabled || state.status == FavoriteSyncStatus::Error {
            return;
        }
        let mut next = state.clone();
        next.operation_epoch = next.operation_epoch.saturating_add(1);
        next.status = FavoriteSyncStatus::Error;
        if next.pending_kind.is_none() {
            set_pending(&mut next, PendingKind::Check, None, None, None);
        }
        next.last_error = Some(message.to_string());
        next.progress_done = 0;
        next.progress_total = 0;
        next.progress_phase = None;
        next.updated_at = now();
        if let Err(error) = self.save_locked(&mut state, next).await {
            tracing::error!(%error, "failed to persist unavailable favorite sync account");
        }
    }

    async fn begin_check(&self, automatic: bool) -> Result<i64, FavoriteSyncError> {
        let session = self.logged_in_session()?;
        let mut state = self.state.lock().await;
        if !state.enabled {
            return Err(FavoriteSyncError::Conflict("收藏同步尚未开启".into()));
        }
        if state.account_id != session.user_id {
            return Err(FavoriteSyncError::Account(
                "当前登录账号与收藏同步绑定账号不一致".into(),
            ));
        }
        let allowed = matches!(
            state.status,
            FavoriteSyncStatus::Synced | FavoriteSyncStatus::NeedsResolution
        );
        if !allowed {
            let message = if automatic {
                "当前收藏同步状态不需要自动检查"
            } else {
                "当前收藏同步状态不能重新检查，请先完成或重试现有任务"
            };
            return Err(FavoriteSyncError::Conflict(message.into()));
        }

        let mut next = state.clone();
        next.status = FavoriteSyncStatus::Checking;
        next.operation_epoch = next.operation_epoch.saturating_add(1);
        set_pending(&mut next, PendingKind::Check, None, None, None);
        next.last_error = None;
        next.progress_done = 0;
        next.progress_total = 0;
        next.progress_phase = None;
        next.updated_at = now();
        self.save_locked(&mut state, next)
            .await
            .map(|record| record.operation_epoch)
            .map_err(internal_error)
    }

    fn spawn_check(self: &Arc<Self>, epoch: i64) {
        self.spawn_pending(epoch, PendingKind::Check);
    }

    fn spawn_pending(self: &Arc<Self>, epoch: i64, kind: PendingKind) {
        let service = self.clone();
        tokio::spawn(async move {
            let _operation = service.operation.lock().await;
            let result = match kind {
                PendingKind::Check => service.run_check(epoch).await,
                PendingKind::Merge => service.run_merge(epoch).await,
                PendingKind::RemoteOverwrite => service.run_remote_overwrite(epoch).await,
                PendingKind::SetFavorite => service.run_pending_target(epoch).await,
            };
            if let Err(error) = result {
                let message = match kind {
                    PendingKind::Check => "读取远端收藏失败，请重试同步",
                    PendingKind::Merge => "合并收藏失败，请重试同步",
                    PendingKind::RemoteOverwrite => "使用远端收藏覆盖本地失败，请重试同步",
                    PendingKind::SetFavorite => "收藏同步失败，请重试同步",
                };
                service.fail_current(epoch, message, &error).await;
            }
        });
    }

    async fn run_check(&self, epoch: i64) -> anyhow::Result<()> {
        self.require_current(epoch).await?;
        let remote = self.fetch_remote_snapshot(epoch).await?;
        let local = self.favorites.all().await?;
        self.require_current(epoch).await?;
        let (local_only, remote_only) = differences(&local, &remote);
        let timestamp = now();
        self.update_if_current(epoch, |record| {
            record.local_count = local.len() as i64;
            record.remote_count = remote.len() as i64;
            record.local_only_count = local_only.len() as i64;
            record.remote_only_count = remote_only.len() as i64;
            record.progress_done = 0;
            record.progress_total = 0;
            record.progress_phase = None;
            record.last_checked_at = Some(timestamp);
            clear_pending(record);
            if local_only.is_empty() && remote_only.is_empty() {
                record.status = FavoriteSyncStatus::Synced;
                record.last_synced_at = Some(timestamp);
            } else {
                record.status = FavoriteSyncStatus::NeedsResolution;
            }
        })
        .await?;
        Ok(())
    }

    async fn run_merge(&self, epoch: i64) -> anyhow::Result<()> {
        self.require_current(epoch).await?;
        self.set_merge_phase(epoch, FavoriteSyncProgressPhase::FetchingRemote, 0)
            .await?;
        let local = self.favorites.all().await?;
        let remote = self.fetch_remote_snapshot(epoch).await?;
        let remote_ids = id_set(&remote);
        let local_only = local
            .iter()
            .filter(|item| !remote_ids.contains(&item.comic_id))
            .cloned()
            .collect::<Vec<_>>();
        self.set_merge_phase(
            epoch,
            FavoriteSyncProgressPhase::UploadingLocal,
            local_only.len() as i64,
        )
        .await?;

        for (index, item) in local_only.iter().enumerate() {
            self.ensure_remote_target(epoch, &item.comic_id, true)
                .await?;
            self.set_progress(epoch, (index + 1) as i64, local_only.len() as i64)
                .await?;
        }

        self.set_merge_phase(epoch, FavoriteSyncProgressPhase::Verifying, 0)
            .await?;
        let remote = self.fetch_remote_snapshot(epoch).await?;
        self.insert_missing_if_current(epoch, &remote).await?;
        let local = self.favorites.all().await?;
        if id_set(&local) != id_set(&remote) {
            anyhow::bail!("favorite sets still differ after merge");
        }
        self.finish_synced(epoch).await
    }

    async fn run_remote_overwrite(&self, epoch: i64) -> anyhow::Result<()> {
        self.require_current(epoch).await?;
        let remote = self.fetch_remote_snapshot(epoch).await?;
        self.replace_all_if_current(epoch, &remote).await?;
        let local = self.favorites.all().await?;
        if id_set(&local) != id_set(&remote) {
            anyhow::bail!("favorite sets still differ after remote overwrite");
        }
        self.finish_synced(epoch).await
    }

    async fn run_pending_target(&self, epoch: i64) -> anyhow::Result<()> {
        let record = self.state.lock().await.clone();
        if !record.enabled || record.operation_epoch != epoch {
            anyhow::bail!("favorite sync task was cancelled");
        }
        let comic_id = record
            .pending_comic_id
            .ok_or_else(|| anyhow::anyhow!("pending favorite operation has no comic id"))?;
        let target = record
            .pending_target
            .ok_or_else(|| anyhow::anyhow!("pending favorite operation has no target"))?;
        self.ensure_remote_target(epoch, &comic_id, target).await?;
        if target {
            let payload = record.pending_payload.ok_or_else(|| {
                anyhow::anyhow!("pending favorite operation has no comic payload")
            })?;
            let input: FavoriteInput = serde_json::from_str(&payload)?;
            self.upsert_local_if_current(epoch, &comic_id, input)
                .await?;
        } else {
            self.remove_local_if_current(epoch, &comic_id).await?;
        }
        self.finish_synced(epoch).await
    }

    async fn fetch_remote_snapshot(&self, epoch: i64) -> anyhow::Result<Vec<FavoriteItem>> {
        let mut page_number = 1;
        let mut expected_total = None;
        let mut seen = HashSet::new();
        let mut items = Vec::new();

        loop {
            self.require_current(epoch).await?;
            if page_number > MAX_REMOTE_PAGES {
                anyhow::bail!("remote favorite pagination exceeded the safety limit");
            }
            let page = self
                .remote
                .favorite_page(page_number, FavoriteOrder::Mr)
                .await?;
            self.require_current(epoch).await?;

            if let Some(total) = expected_total {
                if total != page.total {
                    anyhow::bail!("remote favorite total changed while paging");
                }
            } else {
                expected_total = Some(page.total);
            }

            let page_len = page.items.len();
            let before = seen.len();
            for item in page.items {
                let comic_id = item.id.trim().to_string();
                if comic_id.is_empty()
                    || !comic_id.chars().all(|character| character.is_ascii_digit())
                {
                    anyhow::bail!("remote favorite contains an invalid comic id");
                }
                if seen.insert(comic_id.clone()) {
                    items.push(FavoriteComic {
                        id: comic_id,
                        ..item
                    });
                }
            }

            if page_len > 0 && seen.len() == before {
                anyhow::bail!("remote favorite pagination made no progress");
            }

            let total = expected_total.unwrap_or_default() as usize;
            self.set_progress(epoch, seen.len() as i64, total as i64)
                .await?;
            if total > 0 && seen.len() >= total {
                if seen.len() != total {
                    anyhow::bail!("remote favorite count exceeded the reported total");
                }
                break;
            }
            if page_len < REMOTE_PAGE_SIZE {
                if total > 0 && seen.len() != total {
                    anyhow::bail!("remote favorite snapshot is incomplete");
                }
                break;
            }
            page_number += 1;
        }

        let base_time = now();
        Ok(items
            .into_iter()
            .enumerate()
            .map(|(index, item)| FavoriteItem {
                comic_id: item.id,
                title: item.name,
                author: item.author,
                description: item.description,
                image: item.image,
                tags: item.tags,
                favorited_at: base_time.saturating_sub(index as i64),
            })
            .collect())
    }

    async fn ensure_remote_target(
        &self,
        epoch: i64,
        comic_id: &str,
        target: bool,
    ) -> anyhow::Result<()> {
        self.require_current(epoch).await?;
        if self.remote.favorite_state(comic_id).await? == target {
            return Ok(());
        }

        self.require_current(epoch).await?;
        let toggle_error = self.remote.toggle_favorite(comic_id).await.err();
        self.require_current(epoch).await?;
        match self.remote.favorite_state(comic_id).await {
            Ok(actual) if actual == target => Ok(()),
            Ok(_) => {
                if let Some(error) = toggle_error {
                    Err(error.into())
                } else {
                    anyhow::bail!("remote favorite did not reach the requested state")
                }
            }
            Err(verify_error) => {
                if let Some(toggle_error) = toggle_error {
                    Err(anyhow::anyhow!(
                        "toggle failed ({toggle_error}); verification failed ({verify_error})"
                    ))
                } else {
                    Err(verify_error.into())
                }
            }
        }
    }

    async fn upsert_local_if_current(
        &self,
        epoch: i64,
        comic_id: &str,
        input: FavoriteInput,
    ) -> anyhow::Result<FavoriteItem> {
        let state = self.state.lock().await;
        require_current_record(&state, epoch)?;
        self.favorites.upsert(comic_id, input).await
    }

    async fn remove_local_if_current(&self, epoch: i64, comic_id: &str) -> anyhow::Result<()> {
        let state = self.state.lock().await;
        require_current_record(&state, epoch)?;
        self.favorites.remove(comic_id).await
    }

    async fn insert_missing_if_current(
        &self,
        epoch: i64,
        items: &[FavoriteItem],
    ) -> anyhow::Result<()> {
        let state = self.state.lock().await;
        require_current_record(&state, epoch)?;
        self.favorites.insert_missing(items).await
    }

    async fn replace_all_if_current(
        &self,
        epoch: i64,
        items: &[FavoriteItem],
    ) -> anyhow::Result<()> {
        let state = self.state.lock().await;
        require_current_record(&state, epoch)?;
        self.favorites.replace_all(items).await
    }

    async fn finish_synced(&self, epoch: i64) -> anyhow::Result<()> {
        let count = self.favorites.all().await?.len() as i64;
        let timestamp = now();
        self.update_if_current(epoch, |record| {
            record.status = FavoriteSyncStatus::Synced;
            record.local_count = count;
            record.remote_count = count;
            record.local_only_count = 0;
            record.remote_only_count = 0;
            record.progress_done = 0;
            record.progress_total = 0;
            record.last_checked_at = Some(timestamp);
            record.last_synced_at = Some(timestamp);
            clear_pending(record);
        })
        .await?;
        Ok(())
    }

    async fn set_progress(&self, epoch: i64, done: i64, total: i64) -> anyhow::Result<()> {
        self.update_if_current(epoch, |record| {
            record.progress_done = done;
            record.progress_total = total;
        })
        .await?;
        Ok(())
    }

    async fn set_merge_phase(
        &self,
        epoch: i64,
        phase: FavoriteSyncProgressPhase,
        total: i64,
    ) -> anyhow::Result<()> {
        self.update_if_current(epoch, |record| {
            record.progress_phase = Some(phase);
            record.progress_done = 0;
            record.progress_total = total;
        })
        .await?;
        Ok(())
    }

    async fn fail_current(&self, epoch: i64, message: &str, error: &anyhow::Error) {
        tracing::error!(%error, "favorite synchronization failed");
        if let Err(save_error) = self
            .update_if_current(epoch, |record| {
                record.status = FavoriteSyncStatus::Error;
                record.last_error = Some(message.to_string());
                record.progress_done = 0;
                record.progress_total = 0;
                record.progress_phase = None;
            })
            .await
        {
            tracing::error!(%save_error, "failed to persist favorite synchronization error");
        }
    }

    async fn operation_error(&self, epoch: i64, message: &str) -> FavoriteSyncError {
        let state = self.state.lock().await;
        if !state.enabled || state.operation_epoch != epoch {
            FavoriteSyncError::Conflict("收藏同步已关闭，本次操作未写入本地".into())
        } else {
            FavoriteSyncError::Remote(message.into())
        }
    }

    async fn update_if_current<F>(&self, epoch: i64, update: F) -> anyhow::Result<bool>
    where
        F: FnOnce(&mut SyncRecord),
    {
        let mut state = self.state.lock().await;
        if !state.enabled || state.operation_epoch != epoch {
            return Ok(false);
        }
        let mut next = state.clone();
        update(&mut next);
        next.updated_at = now();
        self.save_locked(&mut state, next).await?;
        Ok(true)
    }

    async fn require_current(&self, epoch: i64) -> anyhow::Result<()> {
        require_current_record(&*self.state.lock().await, epoch)
    }

    async fn save_locked(
        &self,
        state: &mut SyncRecord,
        next: SyncRecord,
    ) -> anyhow::Result<SyncRecord> {
        self.repository.save(&next).await?;
        *state = next.clone();
        Ok(next)
    }

    fn logged_in_session(&self) -> Result<AccountSession, FavoriteSyncError> {
        let session = self.account_rx.borrow().clone();
        if session.login_status != super::account::LoginStatus::LoggedIn
            || session.user_id.is_none()
        {
            return Err(FavoriteSyncError::Account(
                "请先登录 JM 账号再同步收藏".into(),
            ));
        }
        Ok(session)
    }

    async fn validate_order_context(
        &self,
        epoch: i64,
        initial_session: &AccountSession,
        account_rx: &mut watch::Receiver<AccountSession>,
    ) -> Result<(), FavoriteSyncError> {
        // A closed watch channel still retains its last value. Treat closure without an
        // unseen value as stable; any published unseen session still returns `Ok(true)`.
        if account_rx.has_changed().unwrap_or(false) {
            return Err(stale_order_error());
        }
        let current_session = account_rx.borrow_and_update().clone();
        let state = self.state.lock().await;
        if &current_session != initial_session
            || state.operation_epoch != epoch
            || !remote_order_ready(&state, &current_session)
        {
            return Err(stale_order_error());
        }
        Ok(())
    }
}

fn remote_order_ready(record: &SyncRecord, session: &AccountSession) -> bool {
    record.enabled
        && record.status == FavoriteSyncStatus::Synced
        && session.login_status == super::account::LoginStatus::LoggedIn
        && session.user_id.is_some()
        && record.account_id == session.user_id
}

fn stale_order_error() -> FavoriteSyncError {
    FavoriteSyncError::OrderStale("收藏同步状态已变化，正在重新读取".into())
}

fn validate_synced_session(
    record: &SyncRecord,
    session: &AccountSession,
) -> Result<(), FavoriteSyncError> {
    if record.status != FavoriteSyncStatus::Synced {
        return Err(FavoriteSyncError::Conflict(
            "收藏同步尚未就绪，请在设置中完成或重试同步".into(),
        ));
    }
    if record.account_id != session.user_id {
        return Err(FavoriteSyncError::Account(
            "当前登录账号与收藏同步绑定账号不一致".into(),
        ));
    }
    Ok(())
}

fn set_pending(
    record: &mut SyncRecord,
    kind: PendingKind,
    comic_id: Option<String>,
    target: Option<bool>,
    payload: Option<String>,
) {
    record.pending_kind = Some(kind);
    record.pending_comic_id = comic_id;
    record.pending_target = target;
    record.pending_payload = payload;
    record.progress_phase = None;
}

fn clear_pending(record: &mut SyncRecord) {
    record.pending_kind = None;
    record.pending_comic_id = None;
    record.pending_target = None;
    record.pending_payload = None;
    record.progress_phase = None;
    record.last_error = None;
}

fn clear_counts_and_error(record: &mut SyncRecord) {
    record.local_count = 0;
    record.remote_count = 0;
    record.local_only_count = 0;
    record.remote_only_count = 0;
    record.progress_done = 0;
    record.progress_total = 0;
    record.progress_phase = None;
    record.last_error = None;
}

fn reset_disabled(record: &mut SyncRecord) {
    record.enabled = false;
    record.account_id = None;
    record.account_username = None;
    record.status = FavoriteSyncStatus::Disabled;
    clear_counts_and_error(record);
    clear_pending(record);
    record.last_checked_at = None;
    record.last_synced_at = None;
}

fn require_current_record(record: &SyncRecord, epoch: i64) -> anyhow::Result<()> {
    if !record.enabled || record.operation_epoch != epoch {
        anyhow::bail!("favorite sync task was cancelled");
    }
    Ok(())
}

fn differences(
    local: &[FavoriteItem],
    remote: &[FavoriteItem],
) -> (HashSet<String>, HashSet<String>) {
    let local_ids = id_set(local);
    let remote_ids = id_set(remote);
    (
        local_ids.difference(&remote_ids).cloned().collect(),
        remote_ids.difference(&local_ids).cloned().collect(),
    )
}

fn id_set(items: &[FavoriteItem]) -> HashSet<String> {
    items.iter().map(|item| item.comic_id.clone()).collect()
}

fn now() -> i64 {
    Utc::now().timestamp_millis()
}

fn internal_error(error: anyhow::Error) -> FavoriteSyncError {
    tracing::error!(%error, "favorite synchronization storage operation failed");
    FavoriteSyncError::Internal("收藏同步状态保存失败".into())
}

fn local_operation_error(context: &str, error: impl std::fmt::Display) -> FavoriteSyncError {
    tracing::error!(%error, context, "local favorite operation failed");
    FavoriteSyncError::Internal("收藏存储操作失败".into())
}
