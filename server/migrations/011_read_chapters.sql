CREATE TABLE IF NOT EXISTS read_chapters (
    comic_id TEXT NOT NULL,
    chapter_id TEXT NOT NULL,
    read_at INTEGER NOT NULL,
    PRIMARY KEY (comic_id, chapter_id)
);

INSERT OR IGNORE INTO read_chapters (comic_id, chapter_id, read_at)
SELECT comic_id, chapter_id, last_read_at
FROM reading_history
WHERE chapter_id <> '';
