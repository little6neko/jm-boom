use super::{
    remote::{FavoriteRemote, RemoteFuture},
    FavoriteSyncError, FavoriteSyncProgressPhase, FavoriteSyncResolution, FavoriteSyncService,
    FavoriteSyncStatus, PendingKind,
};
use crate::{
    application::{
        account::{AccountSession, LoginStatus},
        FavoriteInput, FavoriteService,
    },
    jm::{FavoriteComic, FavoriteOrder, FavoritePage, JmError},
};
use sqlx::{sqlite::SqlitePoolOptions, SqlitePool};
use std::{
    collections::{BTreeMap, HashSet},
    sync::{
        atomic::{AtomicBool, AtomicU8, AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::sync::{watch, Mutex, Notify};

const FAIL_NONE: u8 = 0;
const FAIL_BEFORE_TOGGLE: u8 = 1;
const FAIL_AFTER_TOGGLE: u8 = 2;

struct FakeRemote {
    favorites: Mutex<BTreeMap<String, FavoriteComic>>,
    fail_next_toggle: AtomicU8,
    fail_next_page: AtomicBool,
    toggle_calls: AtomicUsize,
    page_calls: AtomicUsize,
    reverse_mp: AtomicBool,
    orders: Mutex<Vec<FavoriteOrder>>,
    block_pages: AtomicBool,
    block_toggle: AtomicBool,
    page_started: Notify,
    release_page: Notify,
    toggle_started: Notify,
    release_toggle: Notify,
}

struct BlockingRemote {
    page_started: Notify,
    release_page: Notify,
}

impl FavoriteRemote for BlockingRemote {
    fn favorite_page(&self, _page: u32, _order: FavoriteOrder) -> RemoteFuture<'_, FavoritePage> {
        Box::pin(async move {
            self.page_started.notify_one();
            self.release_page.notified().await;
            Ok(FavoritePage {
                total: 0,
                items: Vec::new(),
            })
        })
    }

    fn favorite_state<'a>(&'a self, _comic_id: &'a str) -> RemoteFuture<'a, bool> {
        Box::pin(async { Ok(false) })
    }

    fn toggle_favorite<'a>(&'a self, _comic_id: &'a str) -> RemoteFuture<'a, ()> {
        Box::pin(async { Ok(()) })
    }
}

impl FakeRemote {
    fn new(items: Vec<FavoriteComic>) -> Self {
        Self {
            favorites: Mutex::new(
                items
                    .into_iter()
                    .map(|item| (item.id.clone(), item))
                    .collect(),
            ),
            fail_next_toggle: AtomicU8::new(FAIL_NONE),
            fail_next_page: AtomicBool::new(false),
            toggle_calls: AtomicUsize::new(0),
            page_calls: AtomicUsize::new(0),
            reverse_mp: AtomicBool::new(false),
            orders: Mutex::new(Vec::new()),
            block_pages: AtomicBool::new(false),
            block_toggle: AtomicBool::new(false),
            page_started: Notify::new(),
            release_page: Notify::new(),
            toggle_started: Notify::new(),
            release_toggle: Notify::new(),
        }
    }

    async fn ids(&self) -> HashSet<String> {
        self.favorites.lock().await.keys().cloned().collect()
    }
}

impl FavoriteRemote for FakeRemote {
    fn favorite_page(&self, page: u32, order: FavoriteOrder) -> RemoteFuture<'_, FavoritePage> {
        Box::pin(async move {
            self.page_calls.fetch_add(1, Ordering::SeqCst);
            self.orders.lock().await.push(order);
            if self.fail_next_page.swap(false, Ordering::SeqCst) {
                return Err(JmError::Network("favorite page failed".into()));
            }
            if self.block_pages.load(Ordering::SeqCst) {
                self.page_started.notify_one();
                self.release_page.notified().await;
            }
            let mut items = self
                .favorites
                .lock()
                .await
                .values()
                .cloned()
                .collect::<Vec<_>>();
            if order == FavoriteOrder::Mp && self.reverse_mp.load(Ordering::SeqCst) {
                items.reverse();
            }
            let offset = (page.saturating_sub(1) as usize) * 20;
            let page_items = items.into_iter().skip(offset).take(20).collect();
            Ok(FavoritePage {
                total: self.favorites.lock().await.len() as u32,
                items: page_items,
            })
        })
    }

    fn favorite_state<'a>(&'a self, comic_id: &'a str) -> RemoteFuture<'a, bool> {
        Box::pin(async move { Ok(self.favorites.lock().await.contains_key(comic_id)) })
    }

    fn toggle_favorite<'a>(&'a self, comic_id: &'a str) -> RemoteFuture<'a, ()> {
        Box::pin(async move {
            self.toggle_calls.fetch_add(1, Ordering::SeqCst);
            if self.block_toggle.load(Ordering::SeqCst) {
                self.toggle_started.notify_one();
                self.release_toggle.notified().await;
            }
            let failure = self.fail_next_toggle.swap(FAIL_NONE, Ordering::SeqCst);
            if failure == FAIL_BEFORE_TOGGLE {
                return Err(JmError::Network("failed before sending".into()));
            }

            let mut favorites = self.favorites.lock().await;
            if favorites.remove(comic_id).is_none() {
                favorites.insert(comic_id.to_string(), remote_item(comic_id));
            }
            drop(favorites);

            if failure == FAIL_AFTER_TOGGLE {
                return Err(JmError::Network("response was lost".into()));
            }
            Ok(())
        })
    }
}

#[tokio::test]
async fn remains_disabled_without_contacting_remote() {
    let (service, remote, local, _) = test_service(vec!["2"], vec!["1"]).await;

    let state = service.state().await;
    assert!(!state.enabled);
    assert_eq!(state.status, FavoriteSyncStatus::Disabled);
    assert_eq!(remote.page_calls.load(Ordering::SeqCst), 0);
    assert_eq!(local_ids(&local).await, id_set(["1"]));
}

#[tokio::test]
async fn uses_local_mr_order_and_rejects_mp_while_sync_is_disabled() {
    let (service, remote, _local, _) = test_service(Vec::new(), vec!["1", "2"]).await;

    let (items, total) = service
        .list(1, 20, FavoriteOrder::Mr)
        .await
        .expect("list local favorites");
    assert_eq!(total, 2);
    assert_eq!(
        items
            .into_iter()
            .map(|item| item.comic_id)
            .collect::<Vec<_>>(),
        ["2", "1"]
    );
    assert!(matches!(
        service.list(1, 20, FavoriteOrder::Mp).await,
        Err(FavoriteSyncError::OrderUnavailable(_))
    ));
    assert_eq!(remote.page_calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn preserves_remote_mr_and_mp_page_order_after_sync() {
    let (service, remote, _local, _) = test_service(vec!["1", "2", "3"], vec!["1", "2", "3"]).await;
    service
        .set_enabled(true)
        .await
        .expect("enable favorite sync");
    wait_for_status(&service, FavoriteSyncStatus::Synced).await;
    assert!(remote
        .orders
        .lock()
        .await
        .iter()
        .all(|order| *order == FavoriteOrder::Mr));
    remote.reverse_mp.store(true, Ordering::SeqCst);
    remote.orders.lock().await.clear();

    let (mr, total) = service
        .list(1, 20, FavoriteOrder::Mr)
        .await
        .expect("list mr favorites");
    assert_eq!(total, 3);
    assert_eq!(
        mr.iter()
            .map(|item| item.comic_id.as_str())
            .collect::<Vec<_>>(),
        ["1", "2", "3"]
    );
    assert!(mr.iter().all(|item| item.title.starts_with("Local ")));

    let (mp, total) = service
        .list(1, 20, FavoriteOrder::Mp)
        .await
        .expect("list mp favorites");
    assert_eq!(total, 3);
    assert_eq!(
        mp.iter()
            .map(|item| item.comic_id.as_str())
            .collect::<Vec<_>>(),
        ["3", "2", "1"]
    );
    assert_eq!(
        remote.orders.lock().await.as_slice(),
        [FavoriteOrder::Mr, FavoriteOrder::Mp]
    );
}

#[tokio::test]
async fn rejects_an_ordered_page_when_remote_and_local_sets_differ() {
    let (service, remote, _local, _) = test_service(vec!["1", "2", "3"], vec!["1", "2", "3"]).await;
    service
        .set_enabled(true)
        .await
        .expect("enable favorite sync");
    wait_for_status(&service, FavoriteSyncStatus::Synced).await;
    let mut favorites = remote.favorites.lock().await;
    favorites.remove("1");
    favorites.insert("4".into(), remote_item("4"));
    drop(favorites);

    assert!(matches!(
        service.list(1, 20, FavoriteOrder::Mr).await,
        Err(FavoriteSyncError::SyncMismatch(_))
    ));
}

#[tokio::test]
async fn rejects_an_ordered_page_when_remote_total_changes() {
    let (service, remote, _local, _) = test_service(vec!["1", "2"], vec!["1", "2"]).await;
    service
        .set_enabled(true)
        .await
        .expect("enable favorite sync");
    wait_for_status(&service, FavoriteSyncStatus::Synced).await;
    remote
        .favorites
        .lock()
        .await
        .insert("3".into(), remote_item("3"));

    assert!(matches!(
        service.list(1, 20, FavoriteOrder::Mp).await,
        Err(FavoriteSyncError::SyncMismatch(_))
    ));
}

#[tokio::test]
async fn does_not_fall_back_to_local_order_after_a_remote_read_failure() {
    let (service, remote, _local, _) = test_service(vec!["1"], vec!["1"]).await;
    service
        .set_enabled(true)
        .await
        .expect("enable favorite sync");
    wait_for_status(&service, FavoriteSyncStatus::Synced).await;
    remote.fail_next_page.store(true, Ordering::SeqCst);

    assert!(matches!(
        service.list(1, 20, FavoriteOrder::Mr).await,
        Err(FavoriteSyncError::Remote(_))
    ));
}

#[tokio::test]
async fn rejects_an_ordered_page_if_sync_changes_during_the_remote_read() {
    let (service, remote, _local, _) = test_service(Vec::new(), Vec::new()).await;
    service
        .set_enabled(true)
        .await
        .expect("enable favorite sync");
    wait_for_status(&service, FavoriteSyncStatus::Synced).await;
    remote.block_pages.store(true, Ordering::SeqCst);

    let list_service = service.clone();
    let list = tokio::spawn(async move { list_service.list(1, 20, FavoriteOrder::Mr).await });
    tokio::time::timeout(Duration::from_secs(1), remote.page_started.notified())
        .await
        .expect("ordered remote page started");
    service
        .set_enabled(false)
        .await
        .expect("disable favorite sync");
    remote.release_page.notify_one();

    assert!(matches!(
        list.await.expect("join ordered page task"),
        Err(FavoriteSyncError::OrderStale(_))
    ));
}

#[tokio::test]
async fn detects_differences_and_merges_the_union() {
    let (service, remote, local, _) = test_service(vec!["2"], vec!["1"]).await;

    service
        .set_enabled(true)
        .await
        .expect("enable favorite sync");
    let state = wait_for_status(&service, FavoriteSyncStatus::NeedsResolution).await;
    assert_eq!(state.local_only_count, 1);
    assert_eq!(state.remote_only_count, 1);

    service
        .resolve(FavoriteSyncResolution::Merge)
        .await
        .expect("start merge");
    let state = wait_for_status(&service, FavoriteSyncStatus::Synced).await;
    assert_eq!(state.local_count, 2);
    assert_eq!(state.remote_count, 2);
    assert_eq!(local_ids(&local).await, id_set(["1", "2"]));
    assert_eq!(remote.ids().await, id_set(["1", "2"]));
    let local_one = local
        .all()
        .await
        .expect("list local favorites")
        .into_iter()
        .find(|item| item.comic_id == "1")
        .expect("local-only item");
    assert_eq!(local_one.title, "Local 1");
}

#[tokio::test]
async fn exposes_merge_progress_phases_in_operation_order() {
    let (service, remote, _local, _) = test_service(vec!["2"], vec!["1"]).await;
    service
        .set_enabled(true)
        .await
        .expect("enable favorite sync");
    wait_for_status(&service, FavoriteSyncStatus::NeedsResolution).await;
    remote.block_pages.store(true, Ordering::SeqCst);
    remote.block_toggle.store(true, Ordering::SeqCst);

    service
        .resolve(FavoriteSyncResolution::Merge)
        .await
        .expect("start merge");
    tokio::time::timeout(Duration::from_secs(1), remote.page_started.notified())
        .await
        .expect("remote fetch started");
    let state = service.state().await;
    assert_eq!(
        state.progress_phase,
        Some(FavoriteSyncProgressPhase::FetchingRemote)
    );
    assert_eq!((state.progress_done, state.progress_total), (0, 0));
    remote.release_page.notify_one();

    tokio::time::timeout(Duration::from_secs(1), remote.toggle_started.notified())
        .await
        .expect("local-only upload started");
    let state = service.state().await;
    assert_eq!(
        state.progress_phase,
        Some(FavoriteSyncProgressPhase::UploadingLocal)
    );
    assert_eq!((state.progress_done, state.progress_total), (0, 1));
    remote.release_toggle.notify_one();

    tokio::time::timeout(Duration::from_secs(1), remote.page_started.notified())
        .await
        .expect("verification fetch started");
    let state = service.state().await;
    assert_eq!(
        state.progress_phase,
        Some(FavoriteSyncProgressPhase::Verifying)
    );
    assert_eq!((state.progress_done, state.progress_total), (0, 0));
    remote.release_page.notify_one();

    let state = wait_for_status(&service, FavoriteSyncStatus::Synced).await;
    assert_eq!(state.progress_phase, None);
}

#[tokio::test]
async fn clears_merge_progress_phase_after_failure() {
    let (service, remote, _local, _) = test_service(vec!["2"], vec!["1"]).await;
    service
        .set_enabled(true)
        .await
        .expect("enable favorite sync");
    wait_for_status(&service, FavoriteSyncStatus::NeedsResolution).await;
    remote.fail_next_page.store(true, Ordering::SeqCst);

    service
        .resolve(FavoriteSyncResolution::Merge)
        .await
        .expect("start merge");
    let state = wait_for_status(&service, FavoriteSyncStatus::Error).await;
    assert_eq!(state.pending_kind, Some(PendingKind::Merge));
    assert_eq!(state.progress_phase, None);
}

#[tokio::test]
async fn exposes_and_retries_an_initial_check_failure() {
    let (service, remote, local, _) = test_service(Vec::new(), vec!["1"]).await;
    remote.fail_next_page.store(true, Ordering::SeqCst);

    service
        .set_enabled(true)
        .await
        .expect("enable favorite sync");
    let state = wait_for_status(&service, FavoriteSyncStatus::Error).await;
    assert_eq!(state.pending_kind, Some(PendingKind::Check));
    assert!(state.last_error.is_some());
    assert_eq!(local_ids(&local).await, id_set(["1"]));

    service.retry().await.expect("retry initial check");
    let state = wait_for_status(&service, FavoriteSyncStatus::NeedsResolution).await;
    assert_eq!(state.local_only_count, 1);
    assert_eq!(local_ids(&local).await, id_set(["1"]));
}

#[tokio::test]
async fn remote_overwrite_replaces_local_without_toggling_remote() {
    let (service, remote, local, _) = test_service(vec!["2"], vec!["1"]).await;
    service
        .set_enabled(true)
        .await
        .expect("enable favorite sync");
    wait_for_status(&service, FavoriteSyncStatus::NeedsResolution).await;

    service
        .resolve(FavoriteSyncResolution::RemoteOverwrite)
        .await
        .expect("start remote overwrite");
    wait_for_status(&service, FavoriteSyncStatus::Synced).await;

    assert_eq!(local_ids(&local).await, id_set(["2"]));
    assert_eq!(remote.ids().await, id_set(["2"]));
    assert_eq!(remote.toggle_calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn reads_every_remote_favorite_page_before_overwrite() {
    let db = test_db().await;
    let local = Arc::new(FavoriteService::new(db.clone()));
    let remote = Arc::new(FakeRemote::new(
        (1..=25).map(|id| remote_item(&id.to_string())).collect(),
    ));
    let (_account_tx, account_rx) = watch::channel(logged_in_session(1, "first"));
    let service = Arc::new(
        FavoriteSyncService::new(db, local.clone(), remote.clone(), account_rx)
            .await
            .expect("create favorite sync service"),
    );

    service
        .set_enabled(true)
        .await
        .expect("enable favorite sync");
    let state = wait_for_status(&service, FavoriteSyncStatus::NeedsResolution).await;
    assert_eq!(state.remote_only_count, 25);
    assert_eq!(remote.page_calls.load(Ordering::SeqCst), 2);

    service
        .resolve(FavoriteSyncResolution::RemoteOverwrite)
        .await
        .expect("start remote overwrite");
    wait_for_status(&service, FavoriteSyncStatus::Synced).await;
    assert_eq!(
        local.all().await.expect("list imported favorites").len(),
        25
    );
}

#[tokio::test]
async fn verifies_target_after_a_lost_toggle_response() {
    let (service, remote, local, _) = test_service(Vec::new(), Vec::new()).await;
    service
        .set_enabled(true)
        .await
        .expect("enable favorite sync");
    wait_for_status(&service, FavoriteSyncStatus::Synced).await;
    remote
        .fail_next_toggle
        .store(FAIL_AFTER_TOGGLE, Ordering::SeqCst);

    service
        .upsert("1", favorite_input("One"))
        .await
        .expect("verify remote target and finish locally");

    assert!(local.contains("1").await.expect("find local favorite"));
    assert_eq!(remote.ids().await, id_set(["1"]));
    assert_eq!(remote.toggle_calls.load(Ordering::SeqCst), 1);
    assert_eq!(service.state().await.status, FavoriteSyncStatus::Synced);
}

#[tokio::test]
async fn removes_a_favorite_remotely_before_deleting_it_locally() {
    let (service, remote, local, _) = test_service(vec!["1"], vec!["1"]).await;
    service
        .set_enabled(true)
        .await
        .expect("enable favorite sync");
    wait_for_status(&service, FavoriteSyncStatus::Synced).await;

    service
        .remove("1")
        .await
        .expect("remove synchronized favorite");

    assert!(!local.contains("1").await.expect("check local favorite"));
    assert!(remote.ids().await.is_empty());
    assert_eq!(remote.toggle_calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn keeps_local_unchanged_and_retries_a_failed_target() {
    let (service, remote, local, _) = test_service(Vec::new(), Vec::new()).await;
    service
        .set_enabled(true)
        .await
        .expect("enable favorite sync");
    wait_for_status(&service, FavoriteSyncStatus::Synced).await;
    remote
        .fail_next_toggle
        .store(FAIL_BEFORE_TOGGLE, Ordering::SeqCst);

    let error = service
        .upsert("1", favorite_input("One"))
        .await
        .expect_err("remote failure must fail local write");
    assert!(matches!(error, FavoriteSyncError::Remote(_)));
    assert!(!local.contains("1").await.expect("check local favorite"));
    let state = service.state().await;
    assert_eq!(state.status, FavoriteSyncStatus::Error);
    assert_eq!(state.pending_kind, Some(PendingKind::SetFavorite));
    assert_eq!(state.pending_comic_id.as_deref(), Some("1"));

    service.retry().await.expect("start retry");
    wait_for_status(&service, FavoriteSyncStatus::Synced).await;
    assert!(local.contains("1").await.expect("find retried favorite"));
    assert_eq!(remote.ids().await, id_set(["1"]));
    assert_eq!(remote.toggle_calls.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn persists_a_failed_target_across_service_restart() {
    let db = test_db().await;
    let local = Arc::new(FavoriteService::new(db.clone()));
    let remote = Arc::new(FakeRemote::new(Vec::new()));
    let (account_tx, account_rx) = watch::channel(logged_in_session(1, "first"));
    let service = Arc::new(
        FavoriteSyncService::new(db.clone(), local.clone(), remote.clone(), account_rx)
            .await
            .expect("create favorite sync service"),
    );
    service
        .set_enabled(true)
        .await
        .expect("enable favorite sync");
    wait_for_status(&service, FavoriteSyncStatus::Synced).await;
    remote
        .fail_next_toggle
        .store(FAIL_BEFORE_TOGGLE, Ordering::SeqCst);
    service
        .upsert("1", favorite_input("One"))
        .await
        .expect_err("persist failed favorite target");
    drop(service);

    let restored = Arc::new(
        FavoriteSyncService::new(db, local.clone(), remote.clone(), account_tx.subscribe())
            .await
            .expect("restore favorite sync service"),
    );
    let state = restored.state().await;
    assert_eq!(state.status, FavoriteSyncStatus::Error);
    assert_eq!(state.pending_kind, Some(PendingKind::SetFavorite));
    assert_eq!(state.pending_comic_id.as_deref(), Some("1"));

    restored.retry().await.expect("retry restored target");
    wait_for_status(&restored, FavoriteSyncStatus::Synced).await;
    assert!(local.contains("1").await.expect("find restored favorite"));
    assert_eq!(remote.ids().await, id_set(["1"]));
}

#[tokio::test]
async fn account_switch_disables_sync_and_preserves_local_favorites() {
    let (service, _remote, local, account_tx) = test_service(vec!["1"], vec!["1"]).await;
    service.start_account_monitor();
    service
        .set_enabled(true)
        .await
        .expect("enable favorite sync");
    wait_for_status(&service, FavoriteSyncStatus::Synced).await;
    assert!(matches!(
        service.clear().await,
        Err(FavoriteSyncError::Conflict(_))
    ));

    account_tx.send_replace(logged_in_session(2, "second"));
    wait_for_status(&service, FavoriteSyncStatus::Disabled).await;

    assert!(local.contains("1").await.expect("preserve local favorite"));
}

#[tokio::test]
async fn disabling_sync_prevents_an_old_background_check_from_writing_state() {
    let db = test_db().await;
    let local = Arc::new(FavoriteService::new(db.clone()));
    let remote = Arc::new(BlockingRemote {
        page_started: Notify::new(),
        release_page: Notify::new(),
    });
    let (_account_tx, account_rx) = watch::channel(logged_in_session(1, "first"));
    let service = Arc::new(
        FavoriteSyncService::new(db, local, remote.clone(), account_rx)
            .await
            .expect("create favorite sync service"),
    );

    service
        .set_enabled(true)
        .await
        .expect("enable favorite sync");
    tokio::time::timeout(Duration::from_secs(1), remote.page_started.notified())
        .await
        .expect("background page request started");
    service
        .set_enabled(false)
        .await
        .expect("disable favorite sync");
    remote.release_page.notify_one();
    tokio::time::sleep(Duration::from_millis(20)).await;

    let state = service.state().await;
    assert!(!state.enabled);
    assert_eq!(state.status, FavoriteSyncStatus::Disabled);
    assert!(state.last_error.is_none());
}

#[test]
fn serializes_the_frontend_favorite_sync_contract() {
    let state = super::FavoriteSyncState {
        enabled: true,
        status: FavoriteSyncStatus::NeedsResolution,
        account_username: Some("tester".into()),
        local_count: 2,
        remote_count: 3,
        local_only_count: 1,
        remote_only_count: 2,
        progress_done: 0,
        progress_total: 0,
        progress_phase: Some(FavoriteSyncProgressPhase::Verifying),
        pending_kind: None,
        pending_comic_id: None,
        pending_target: None,
        last_error: None,
        last_checked_at: Some(1),
        last_synced_at: None,
    };

    let value = serde_json::to_value(state).expect("serialize favorite sync state");
    assert_eq!(value["status"], "needsResolution");
    assert_eq!(value["accountUsername"], "tester");
    assert_eq!(value["localOnlyCount"], 1);
    assert_eq!(value["remoteOnlyCount"], 2);
    assert_eq!(value["progressPhase"], "verifying");
}

async fn test_service(
    remote_ids: Vec<&str>,
    local_ids: Vec<&str>,
) -> (
    Arc<FavoriteSyncService>,
    Arc<FakeRemote>,
    Arc<FavoriteService>,
    watch::Sender<AccountSession>,
) {
    let db = test_db().await;
    let local = Arc::new(FavoriteService::new(db.clone()));
    for id in local_ids {
        local
            .upsert(id, favorite_input(&format!("Local {id}")))
            .await
            .expect("seed local favorite");
    }
    let remote = Arc::new(FakeRemote::new(
        remote_ids.into_iter().map(remote_item).collect(),
    ));
    let (account_tx, account_rx) = watch::channel(logged_in_session(1, "first"));
    let service = Arc::new(
        FavoriteSyncService::new(db, local.clone(), remote.clone(), account_rx)
            .await
            .expect("create favorite sync service"),
    );
    (service, remote, local, account_tx)
}

async fn test_db() -> SqlitePool {
    let db = SqlitePoolOptions::new()
        .max_connections(1)
        .connect("sqlite::memory:")
        .await
        .expect("connect in-memory sqlite");
    sqlx::migrate!("./migrations")
        .run(&db)
        .await
        .expect("run migrations");
    db
}

async fn wait_for_status(
    service: &FavoriteSyncService,
    expected: FavoriteSyncStatus,
) -> super::FavoriteSyncState {
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let state = service.state().await;
            if state.status == expected {
                return state;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap_or_else(|_| panic!("timed out waiting for favorite sync status {expected:?}"))
}

async fn local_ids(local: &FavoriteService) -> HashSet<String> {
    local
        .all()
        .await
        .expect("list local favorites")
        .into_iter()
        .map(|item| item.comic_id)
        .collect()
}

fn id_set<const N: usize>(ids: [&str; N]) -> HashSet<String> {
    ids.into_iter().map(str::to_string).collect()
}

fn favorite_input(title: &str) -> FavoriteInput {
    FavoriteInput {
        title: title.into(),
        author: "Author".into(),
        description: "Description".into(),
        image: "cover.jpg".into(),
        tags: vec!["Tag".into()],
    }
}

fn remote_item(id: &str) -> FavoriteComic {
    FavoriteComic {
        id: id.into(),
        name: format!("Remote {id}"),
        author: "Remote Author".into(),
        description: "Remote Description".into(),
        image: "remote.jpg".into(),
        tags: vec!["Remote Tag".into()],
    }
}

fn logged_in_session(user_id: u32, username: &str) -> AccountSession {
    AccountSession {
        user_id: Some(user_id),
        username: Some(username.into()),
        login_status: LoginStatus::LoggedIn,
    }
}
