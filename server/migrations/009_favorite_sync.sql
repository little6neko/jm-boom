CREATE TABLE IF NOT EXISTS favorite_sync_state (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    enabled INTEGER NOT NULL DEFAULT 0,
    account_id INTEGER,
    account_username TEXT,
    status TEXT NOT NULL DEFAULT 'disabled',
    local_count INTEGER NOT NULL DEFAULT 0,
    remote_count INTEGER NOT NULL DEFAULT 0,
    local_only_count INTEGER NOT NULL DEFAULT 0,
    remote_only_count INTEGER NOT NULL DEFAULT 0,
    progress_done INTEGER NOT NULL DEFAULT 0,
    progress_total INTEGER NOT NULL DEFAULT 0,
    operation_epoch INTEGER NOT NULL DEFAULT 0,
    pending_kind TEXT,
    pending_comic_id TEXT,
    pending_target INTEGER,
    pending_payload TEXT,
    last_error TEXT,
    last_checked_at INTEGER,
    last_synced_at INTEGER,
    updated_at INTEGER NOT NULL
);

INSERT OR IGNORE INTO favorite_sync_state (
    id,
    enabled,
    status,
    updated_at
) VALUES (
    1,
    0,
    'disabled',
    0
);
