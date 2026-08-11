use super::{FavoriteSyncProgressPhase, FavoriteSyncStatus, PendingKind};
use sqlx::{sqlite::SqliteRow, Row, SqlitePool};

#[derive(Clone, Debug)]
pub(super) struct SyncRecord {
    pub enabled: bool,
    pub account_id: Option<u32>,
    pub account_username: Option<String>,
    pub status: FavoriteSyncStatus,
    pub local_count: i64,
    pub remote_count: i64,
    pub local_only_count: i64,
    pub remote_only_count: i64,
    pub progress_done: i64,
    pub progress_total: i64,
    pub progress_phase: Option<FavoriteSyncProgressPhase>,
    pub operation_epoch: i64,
    pub pending_kind: Option<PendingKind>,
    pub pending_comic_id: Option<String>,
    pub pending_target: Option<bool>,
    pub pending_payload: Option<String>,
    pub last_error: Option<String>,
    pub last_checked_at: Option<i64>,
    pub last_synced_at: Option<i64>,
    pub updated_at: i64,
}

#[derive(Clone)]
pub(super) struct FavoriteSyncRepository {
    db: SqlitePool,
}

impl FavoriteSyncRepository {
    pub fn new(db: SqlitePool) -> Self {
        Self { db }
    }

    pub async fn load(&self) -> anyhow::Result<SyncRecord> {
        let row = sqlx::query(
            "SELECT enabled, account_id, account_username, status, local_count, remote_count, \
             local_only_count, remote_only_count, progress_done, progress_total, progress_phase, \
             operation_epoch, pending_kind, pending_comic_id, pending_target, pending_payload, last_error, \
             last_checked_at, last_synced_at, updated_at \
             FROM favorite_sync_state WHERE id = 1",
        )
        .fetch_one(&self.db)
        .await?;
        decode_record(row)
    }

    pub async fn save(&self, record: &SyncRecord) -> anyhow::Result<()> {
        sqlx::query(
            "UPDATE favorite_sync_state SET \
             enabled = ?, account_id = ?, account_username = ?, status = ?, \
             local_count = ?, remote_count = ?, local_only_count = ?, remote_only_count = ?, \
             progress_done = ?, progress_total = ?, progress_phase = ?, operation_epoch = ?, pending_kind = ?, \
             pending_comic_id = ?, pending_target = ?, pending_payload = ?, last_error = ?, \
             last_checked_at = ?, last_synced_at = ?, updated_at = ? WHERE id = 1",
        )
        .bind(record.enabled)
        .bind(record.account_id.map(i64::from))
        .bind(&record.account_username)
        .bind(record.status.db_value())
        .bind(record.local_count)
        .bind(record.remote_count)
        .bind(record.local_only_count)
        .bind(record.remote_only_count)
        .bind(record.progress_done)
        .bind(record.progress_total)
        .bind(record.progress_phase.map(FavoriteSyncProgressPhase::db_value))
        .bind(record.operation_epoch)
        .bind(record.pending_kind.map(PendingKind::db_value))
        .bind(&record.pending_comic_id)
        .bind(record.pending_target)
        .bind(&record.pending_payload)
        .bind(&record.last_error)
        .bind(record.last_checked_at)
        .bind(record.last_synced_at)
        .bind(record.updated_at)
        .execute(&self.db)
        .await?;
        Ok(())
    }
}

fn decode_record(row: SqliteRow) -> anyhow::Result<SyncRecord> {
    let account_id = row
        .try_get::<Option<i64>, _>("account_id")?
        .map(u32::try_from)
        .transpose()?;
    let status = FavoriteSyncStatus::from_db(row.try_get("status")?)?;
    let pending_kind = row
        .try_get::<Option<String>, _>("pending_kind")?
        .map(|value| PendingKind::from_db(&value))
        .transpose()?;
    let progress_phase = row
        .try_get::<Option<String>, _>("progress_phase")?
        .map(|value| FavoriteSyncProgressPhase::from_db(&value))
        .transpose()?;

    Ok(SyncRecord {
        enabled: row.try_get::<i64, _>("enabled")? != 0,
        account_id,
        account_username: row.try_get("account_username")?,
        status,
        local_count: row.try_get("local_count")?,
        remote_count: row.try_get("remote_count")?,
        local_only_count: row.try_get("local_only_count")?,
        remote_only_count: row.try_get("remote_only_count")?,
        progress_done: row.try_get("progress_done")?,
        progress_total: row.try_get("progress_total")?,
        progress_phase,
        operation_epoch: row.try_get("operation_epoch")?,
        pending_kind,
        pending_comic_id: row.try_get("pending_comic_id")?,
        pending_target: row
            .try_get::<Option<i64>, _>("pending_target")?
            .map(|value| value != 0),
        pending_payload: row.try_get("pending_payload")?,
        last_error: row.try_get("last_error")?,
        last_checked_at: row.try_get("last_checked_at")?,
        last_synced_at: row.try_get("last_synced_at")?,
        updated_at: row.try_get("updated_at")?,
    })
}
