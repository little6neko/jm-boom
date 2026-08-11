use serde::Deserialize;
use sqlx::{QueryBuilder, Sqlite, SqlitePool};

#[derive(Debug, Clone)]
pub struct ReadingHistoryItem {
    pub comic_id: String,
    pub title: String,
    pub author: String,
    pub image: String,
    pub chapter_id: String,
    pub chapter_title: String,
    pub page_index: i64,
    pub page_count: i64,
    pub last_read_at: i64,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReadingHistoryInput {
    pub title: String,
    pub author: String,
    pub image: String,
    pub chapter_id: String,
    pub chapter_title: String,
    pub page_index: i64,
    pub page_count: i64,
    #[serde(default)]
    pub last_read_at: Option<i64>,
}

#[derive(Clone)]
pub struct ReadingHistoryService {
    db: SqlitePool,
}

impl ReadingHistoryService {
    pub fn new(db: SqlitePool) -> Self {
        Self { db }
    }

    pub async fn list(
        &self,
        page: u32,
        page_size: u32,
    ) -> anyhow::Result<(Vec<ReadingHistoryItem>, i64)> {
        let total = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM reading_history")
            .fetch_one(&self.db)
            .await?;
        let offset = i64::from(page - 1) * i64::from(page_size);
        let rows = sqlx::query_as::<
            _,
            (
                String,
                String,
                String,
                String,
                String,
                String,
                i64,
                i64,
                i64,
            ),
        >(
            "SELECT comic_id, title, author, image, chapter_id, chapter_title, \
             page_index, page_count, last_read_at \
             FROM reading_history ORDER BY last_read_at DESC, comic_id DESC LIMIT ? OFFSET ?",
        )
        .bind(i64::from(page_size))
        .bind(offset)
        .fetch_all(&self.db)
        .await?;

        let items = rows
            .into_iter()
            .map(
                |(
                    comic_id,
                    title,
                    author,
                    image,
                    chapter_id,
                    chapter_title,
                    page_index,
                    page_count,
                    last_read_at,
                )| ReadingHistoryItem {
                    comic_id,
                    title,
                    author,
                    image,
                    chapter_id,
                    chapter_title,
                    page_index,
                    page_count,
                    last_read_at,
                },
            )
            .collect();
        Ok((items, total))
    }

    pub async fn get(&self, comic_id: &str) -> anyhow::Result<Option<ReadingHistoryItem>> {
        let row = sqlx::query_as::<
            _,
            (
                String,
                String,
                String,
                String,
                String,
                String,
                i64,
                i64,
                i64,
            ),
        >(
            "SELECT comic_id, title, author, image, chapter_id, chapter_title, \
             page_index, page_count, last_read_at \
             FROM reading_history WHERE comic_id = ?",
        )
        .bind(comic_id)
        .fetch_optional(&self.db)
        .await?;

        Ok(row.map(
            |(
                comic_id,
                title,
                author,
                image,
                chapter_id,
                chapter_title,
                page_index,
                page_count,
                last_read_at,
            )| ReadingHistoryItem {
                comic_id,
                title,
                author,
                image,
                chapter_id,
                chapter_title,
                page_index,
                page_count,
                last_read_at,
            },
        ))
    }

    pub async fn read_chapter_ids(&self, comic_id: &str) -> anyhow::Result<Vec<String>> {
        sqlx::query_scalar::<_, String>(
            "SELECT chapter_id FROM read_chapters \
             WHERE comic_id = ? ORDER BY read_at ASC, chapter_id ASC",
        )
        .bind(comic_id)
        .fetch_all(&self.db)
        .await
        .map_err(Into::into)
    }

    pub async fn upsert(&self, comic_id: &str, input: ReadingHistoryInput) -> anyhow::Result<()> {
        let last_read_at = input
            .last_read_at
            .unwrap_or_else(|| chrono::Utc::now().timestamp_millis());
        let chapter_id = input.chapter_id.clone();
        let mut transaction = self.db.begin().await?;
        persist_item(&mut transaction, comic_id, input, last_read_at).await?;
        persist_read_chapter(&mut transaction, comic_id, &chapter_id, last_read_at).await?;
        transaction.commit().await?;
        Ok(())
    }

    pub async fn remove_many(&self, comic_ids: &[String]) -> anyhow::Result<()> {
        if comic_ids.is_empty() {
            return Ok(());
        }

        let mut transaction = self.db.begin().await?;
        let mut read_chapters_query =
            QueryBuilder::<Sqlite>::new("DELETE FROM read_chapters WHERE comic_id IN (");
        {
            let mut ids = read_chapters_query.separated(", ");
            for comic_id in comic_ids {
                ids.push_bind(comic_id);
            }
        }
        read_chapters_query
            .push(")")
            .build()
            .execute(&mut *transaction)
            .await?;

        let mut history_query =
            QueryBuilder::<Sqlite>::new("DELETE FROM reading_history WHERE comic_id IN (");
        {
            let mut ids = history_query.separated(", ");
            for comic_id in comic_ids {
                ids.push_bind(comic_id);
            }
        }
        history_query
            .push(")")
            .build()
            .execute(&mut *transaction)
            .await?;
        transaction.commit().await?;
        Ok(())
    }

    pub async fn clear(&self) -> anyhow::Result<()> {
        let mut transaction = self.db.begin().await?;
        sqlx::query("DELETE FROM read_chapters")
            .execute(&mut *transaction)
            .await?;
        sqlx::query("DELETE FROM reading_history")
            .execute(&mut *transaction)
            .await?;
        transaction.commit().await?;
        Ok(())
    }
}

async fn persist_item(
    transaction: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    comic_id: &str,
    input: ReadingHistoryInput,
    last_read_at: i64,
) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT INTO reading_history \
         (comic_id, title, author, image, chapter_id, chapter_title, \
          page_index, page_count, last_read_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?) \
         ON CONFLICT(comic_id) DO UPDATE SET \
         title = excluded.title, author = excluded.author, image = excluded.image, \
         chapter_id = excluded.chapter_id, chapter_title = excluded.chapter_title, \
         page_index = excluded.page_index, page_count = excluded.page_count, \
         last_read_at = excluded.last_read_at \
         WHERE excluded.last_read_at >= reading_history.last_read_at",
    )
    .bind(comic_id)
    .bind(input.title)
    .bind(input.author)
    .bind(input.image)
    .bind(input.chapter_id)
    .bind(input.chapter_title)
    .bind(input.page_index)
    .bind(input.page_count)
    .bind(last_read_at)
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

async fn persist_read_chapter(
    transaction: &mut sqlx::Transaction<'_, sqlx::Sqlite>,
    comic_id: &str,
    chapter_id: &str,
    read_at: i64,
) -> anyhow::Result<()> {
    sqlx::query(
        "INSERT INTO read_chapters (comic_id, chapter_id, read_at) VALUES (?, ?, ?) \
         ON CONFLICT(comic_id, chapter_id) DO UPDATE SET read_at = excluded.read_at \
         WHERE excluded.read_at >= read_chapters.read_at",
    )
    .bind(comic_id)
    .bind(chapter_id)
    .bind(read_at)
    .execute(&mut **transaction)
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{ReadingHistoryInput, ReadingHistoryService};
    use sqlx::sqlite::SqlitePoolOptions;
    use std::collections::HashSet;

    #[tokio::test]
    async fn newer_progress_wins_and_items_are_ordered_by_last_read_time() {
        let service = test_service().await;
        service
            .upsert("1", test_input("chapter-old", 1, 10))
            .await
            .expect("insert old progress");
        service
            .upsert("2", test_input("chapter-two", 2, 20))
            .await
            .expect("insert second progress");
        service
            .upsert("1", test_input("chapter-new", 3, 30))
            .await
            .expect("update progress");

        let (items, total) = service.list(1, 1).await.expect("list first page");
        assert_eq!(total, 2);
        assert_eq!(items[0].comic_id, "1");
        assert_eq!(items[0].chapter_id, "chapter-new");
        assert_eq!(items[0].page_index, 3);
        let (items, total) = service.list(2, 1).await.expect("list second page");
        assert_eq!(total, 2);
        assert_eq!(items[0].comic_id, "2");
        let item = service
            .get("1")
            .await
            .expect("get history")
            .expect("history item");
        assert_eq!(item.chapter_id, "chapter-new");
        assert_eq!(item.page_index, 3);
        assert_eq!(
            service
                .read_chapter_ids("1")
                .await
                .expect("get read chapters")
                .into_iter()
                .collect::<HashSet<_>>(),
            HashSet::from(["chapter-old".to_string(), "chapter-new".to_string()])
        );
        assert!(service.get("3").await.expect("missing history").is_none());
    }

    #[tokio::test]
    async fn tracks_only_exact_read_chapters_and_deduplicates_repeated_reads() {
        let service = test_service().await;
        service
            .upsert("1", test_input("12", 1, 10))
            .await
            .expect("read chapter 12");
        service
            .upsert("1", test_input("15", 1, 20))
            .await
            .expect("read chapter 15");
        service
            .upsert("1", test_input("12", 2, 30))
            .await
            .expect("reread chapter 12");

        let read_chapter_ids = service
            .read_chapter_ids("1")
            .await
            .expect("get read chapters");
        assert_eq!(read_chapter_ids, vec!["15".to_string(), "12".to_string()]);
        assert!(!read_chapter_ids.contains(&"11".to_string()));

        let chapter_count =
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM read_chapters WHERE comic_id = '1'")
                .fetch_one(&service.db)
                .await
                .expect("count exact read chapters");
        assert_eq!(chapter_count, 2);
    }

    #[tokio::test]
    async fn remove_many_with_empty_ids_is_a_noop() {
        let service = test_service().await;
        service
            .upsert("1", test_input("chapter-one", 1, 10))
            .await
            .expect("insert history");

        service
            .remove_many(&[])
            .await
            .expect("remove empty history selection");

        assert!(service.get("1").await.expect("get history").is_some());
        assert_eq!(
            service
                .read_chapter_ids("1")
                .await
                .expect("get read chapters"),
            vec!["chapter-one"]
        );
    }

    #[tokio::test]
    async fn remove_many_deletes_requested_items_and_ignores_missing_ids() {
        let service = test_service().await;
        for (comic_id, last_read_at) in [("1", 10), ("2", 20), ("3", 30)] {
            service
                .upsert(
                    comic_id,
                    test_input(&format!("chapter-{comic_id}"), 1, last_read_at),
                )
                .await
                .expect("insert history");
        }
        let comic_ids = vec!["1".to_string(), "3".to_string(), "404".to_string()];

        service
            .remove_many(&comic_ids)
            .await
            .expect("remove selected history");

        assert!(service.get("1").await.expect("get first history").is_none());
        assert!(service.get("3").await.expect("get third history").is_none());
        assert!(service
            .get("404")
            .await
            .expect("get missing history")
            .is_none());
        assert!(service.get("2").await.expect("get kept history").is_some());
        let (items, total) = service.list(1, 20).await.expect("list remaining history");
        assert_eq!(total, 1);
        assert_eq!(items[0].comic_id, "2");
        assert!(service
            .read_chapter_ids("1")
            .await
            .expect("get removed read chapters")
            .is_empty());
        assert_eq!(
            service
                .read_chapter_ids("2")
                .await
                .expect("get kept read chapters"),
            vec!["chapter-2"]
        );
    }

    #[tokio::test]
    async fn clear_removes_history_and_all_read_chapters() {
        let service = test_service().await;
        service
            .upsert("1", test_input("12", 1, 10))
            .await
            .expect("insert first history");
        service
            .upsert("2", test_input("20", 1, 20))
            .await
            .expect("insert second history");

        service.clear().await.expect("clear history");

        assert_eq!(service.list(1, 20).await.expect("list history").1, 0);
        let read_chapter_count = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM read_chapters")
            .fetch_one(&service.db)
            .await
            .expect("count read chapters");
        assert_eq!(read_chapter_count, 0);
    }

    #[tokio::test]
    async fn migration_imports_only_the_explicit_chapter_from_existing_history() {
        let db = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("connect in-memory sqlite");
        sqlx::raw_sql(include_str!("../../migrations/007_reading_history.sql"))
            .execute(&db)
            .await
            .expect("create legacy reading history");
        sqlx::query(
            "INSERT INTO reading_history \
             (comic_id, title, author, image, chapter_id, chapter_title, \
              page_index, page_count, last_read_at) \
             VALUES ('1', '', '', '', '12', '', 0, 1, 10), \
                    ('2', '', '', '', '25', '', 0, 1, 20), \
                    ('3', '', '', '', '', '', 0, 1, 30)",
        )
        .execute(&db)
        .await
        .expect("insert legacy history");

        sqlx::raw_sql(include_str!("../../migrations/011_read_chapters.sql"))
            .execute(&db)
            .await
            .expect("migrate read chapters");

        let migrated = sqlx::query_as::<_, (String, String, i64)>(
            "SELECT comic_id, chapter_id, read_at FROM read_chapters ORDER BY comic_id",
        )
        .fetch_all(&db)
        .await
        .expect("list migrated read chapters");
        assert_eq!(
            migrated,
            vec![
                ("1".to_string(), "12".to_string(), 10),
                ("2".to_string(), "25".to_string(), 20)
            ]
        );
    }

    async fn test_service() -> ReadingHistoryService {
        let db = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("connect in-memory sqlite");
        sqlx::migrate!("./migrations")
            .run(&db)
            .await
            .expect("run migrations");
        ReadingHistoryService::new(db)
    }

    fn test_input(chapter_id: &str, page_index: i64, last_read_at: i64) -> ReadingHistoryInput {
        ReadingHistoryInput {
            title: "Title".into(),
            author: "Author".into(),
            image: "cover.jpg".into(),
            chapter_id: chapter_id.into(),
            chapter_title: "Chapter".into(),
            page_index,
            page_count: 10,
            last_read_at: Some(last_read_at),
        }
    }
}
