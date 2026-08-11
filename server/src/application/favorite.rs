use serde::{Deserialize, Serialize};
use sqlx::{QueryBuilder, Sqlite, SqlitePool};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct FavoriteItem {
    pub comic_id: String,
    pub title: String,
    pub author: String,
    pub description: String,
    pub image: String,
    pub tags: Vec<String>,
    pub favorited_at: i64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FavoriteInput {
    pub title: String,
    pub author: String,
    pub description: String,
    pub image: String,
    pub tags: Vec<String>,
}

#[derive(Clone)]
pub struct FavoriteService {
    db: SqlitePool,
}

impl FavoriteService {
    pub fn new(db: SqlitePool) -> Self {
        Self { db }
    }

    pub async fn list(
        &self,
        page: u32,
        page_size: u32,
    ) -> anyhow::Result<(Vec<FavoriteItem>, i64)> {
        let total = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM favorites")
            .fetch_one(&self.db)
            .await?;
        let offset = i64::from(page - 1) * i64::from(page_size);
        let rows = sqlx::query_as::<_, (String, String, String, String, String, String, i64)>(
            "SELECT comic_id, title, author, description, image, tags, favorited_at \
             FROM favorites ORDER BY favorited_at DESC, comic_id DESC LIMIT ? OFFSET ?",
        )
        .bind(i64::from(page_size))
        .bind(offset)
        .fetch_all(&self.db)
        .await?;

        let items = rows
            .into_iter()
            .map(
                |(comic_id, title, author, description, image, tags, favorited_at)| {
                    Ok(FavoriteItem {
                        comic_id,
                        title,
                        author,
                        description,
                        image,
                        tags: serde_json::from_str(&tags)?,
                        favorited_at,
                    })
                },
            )
            .collect::<anyhow::Result<Vec<_>>>()?;
        Ok((items, total))
    }

    pub async fn count(&self) -> anyhow::Result<i64> {
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM favorites")
            .fetch_one(&self.db)
            .await
            .map_err(Into::into)
    }

    pub async fn by_ids_in_order(&self, comic_ids: &[String]) -> anyhow::Result<Vec<FavoriteItem>> {
        if comic_ids.is_empty() {
            return Ok(Vec::new());
        }

        let mut query = QueryBuilder::<Sqlite>::new(
            "SELECT comic_id, title, author, description, image, tags, favorited_at \
             FROM favorites WHERE comic_id IN (",
        );
        let mut separated = query.separated(", ");
        for comic_id in comic_ids {
            separated.push_bind(comic_id);
        }
        separated.push_unseparated(")");

        let rows = query
            .build_query_as::<(String, String, String, String, String, String, i64)>()
            .fetch_all(&self.db)
            .await?;
        let mut items_by_id = rows
            .into_iter()
            .map(
                |(comic_id, title, author, description, image, tags, favorited_at)| {
                    let item = FavoriteItem {
                        comic_id: comic_id.clone(),
                        title,
                        author,
                        description,
                        image,
                        tags: serde_json::from_str(&tags)?,
                        favorited_at,
                    };
                    Ok((comic_id, item))
                },
            )
            .collect::<anyhow::Result<HashMap<_, _>>>()?;

        Ok(comic_ids
            .iter()
            .filter_map(|comic_id| items_by_id.remove(comic_id))
            .collect())
    }

    pub async fn contains(&self, comic_id: &str) -> anyhow::Result<bool> {
        let exists = sqlx::query_scalar::<_, i64>(
            "SELECT EXISTS(SELECT 1 FROM favorites WHERE comic_id = ?)",
        )
        .bind(comic_id)
        .fetch_one(&self.db)
        .await?;
        Ok(exists != 0)
    }

    pub async fn all(&self) -> anyhow::Result<Vec<FavoriteItem>> {
        let rows = sqlx::query_as::<_, (String, String, String, String, String, String, i64)>(
            "SELECT comic_id, title, author, description, image, tags, favorited_at \
             FROM favorites ORDER BY favorited_at DESC, comic_id DESC",
        )
        .fetch_all(&self.db)
        .await?;

        rows.into_iter()
            .map(
                |(comic_id, title, author, description, image, tags, favorited_at)| {
                    Ok(FavoriteItem {
                        comic_id,
                        title,
                        author,
                        description,
                        image,
                        tags: serde_json::from_str(&tags)?,
                        favorited_at,
                    })
                },
            )
            .collect()
    }

    pub async fn upsert(
        &self,
        comic_id: &str,
        input: FavoriteInput,
    ) -> anyhow::Result<FavoriteItem> {
        let favorited_at = chrono::Utc::now().timestamp_millis();
        let FavoriteInput {
            title,
            author,
            description,
            image,
            tags,
        } = input;
        let serialized_tags = serde_json::to_string(&tags)?;
        sqlx::query(
            "INSERT INTO favorites \
             (comic_id, title, author, description, image, tags, favorited_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?) \
             ON CONFLICT(comic_id) DO UPDATE SET \
             title = excluded.title, author = excluded.author, \
             description = excluded.description, image = excluded.image, \
             tags = excluded.tags, favorited_at = excluded.favorited_at",
        )
        .bind(comic_id)
        .bind(&title)
        .bind(&author)
        .bind(&description)
        .bind(&image)
        .bind(serialized_tags)
        .bind(favorited_at)
        .execute(&self.db)
        .await?;
        Ok(FavoriteItem {
            comic_id: comic_id.to_string(),
            title,
            author,
            description,
            image,
            tags,
            favorited_at,
        })
    }

    pub async fn remove(&self, comic_id: &str) -> anyhow::Result<()> {
        sqlx::query("DELETE FROM favorites WHERE comic_id = ?")
            .bind(comic_id)
            .execute(&self.db)
            .await?;
        Ok(())
    }

    pub async fn clear(&self) -> anyhow::Result<()> {
        sqlx::query("DELETE FROM favorites")
            .execute(&self.db)
            .await?;
        Ok(())
    }

    pub async fn insert_missing(&self, items: &[FavoriteItem]) -> anyhow::Result<()> {
        let mut transaction = self.db.begin().await?;
        for item in items {
            sqlx::query(
                "INSERT OR IGNORE INTO favorites \
                 (comic_id, title, author, description, image, tags, favorited_at) \
                 VALUES (?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(&item.comic_id)
            .bind(&item.title)
            .bind(&item.author)
            .bind(&item.description)
            .bind(&item.image)
            .bind(serde_json::to_string(&item.tags)?)
            .bind(item.favorited_at)
            .execute(&mut *transaction)
            .await?;
        }
        transaction.commit().await?;
        Ok(())
    }

    pub async fn replace_all(&self, items: &[FavoriteItem]) -> anyhow::Result<()> {
        let mut transaction = self.db.begin().await?;
        sqlx::query("DELETE FROM favorites")
            .execute(&mut *transaction)
            .await?;
        for item in items {
            sqlx::query(
                "INSERT INTO favorites \
                 (comic_id, title, author, description, image, tags, favorited_at) \
                 VALUES (?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(&item.comic_id)
            .bind(&item.title)
            .bind(&item.author)
            .bind(&item.description)
            .bind(&item.image)
            .bind(serde_json::to_string(&item.tags)?)
            .bind(item.favorited_at)
            .execute(&mut *transaction)
            .await?;
        }
        transaction.commit().await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{FavoriteInput, FavoriteService};
    use sqlx::sqlite::SqlitePoolOptions;

    #[tokio::test]
    async fn persists_orders_and_removes_instance_favorites() {
        let service = test_service().await;
        service
            .upsert("1", test_input("One"))
            .await
            .expect("insert first favorite");
        service
            .upsert("2", test_input("Two"))
            .await
            .expect("insert second favorite");

        let (items, total) = service.list(1, 1).await.expect("list first page");
        assert_eq!(total, 2);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].comic_id, "2");
        let (items, total) = service.list(2, 1).await.expect("list second page");
        assert_eq!(total, 2);
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].comic_id, "1");
        assert!(service.contains("1").await.expect("find favorite"));
        assert!(!service.contains("3").await.expect("miss favorite"));

        service.remove("2").await.expect("remove favorite");
        let (items, total) = service.list(1, 20).await.expect("list after remove");
        assert_eq!(total, 1);
        assert_eq!(items.len(), 1);

        service.clear().await.expect("clear favorites");
        let (items, total) = service.list(1, 20).await.expect("list after clear");
        assert_eq!(total, 0);
        assert!(items.is_empty());
    }

    #[tokio::test]
    async fn inserts_only_missing_items_and_replaces_in_one_operation() {
        let service = test_service().await;
        let original = service
            .upsert("1", test_input("Local"))
            .await
            .expect("insert local favorite");
        service
            .insert_missing(&[
                super::FavoriteItem {
                    comic_id: "1".into(),
                    title: "Remote title must not replace local".into(),
                    author: "Remote".into(),
                    description: String::new(),
                    image: String::new(),
                    tags: Vec::new(),
                    favorited_at: original.favorited_at + 100,
                },
                super::FavoriteItem {
                    comic_id: "2".into(),
                    title: "Remote only".into(),
                    author: "Remote".into(),
                    description: String::new(),
                    image: String::new(),
                    tags: Vec::new(),
                    favorited_at: original.favorited_at + 50,
                },
            ])
            .await
            .expect("merge missing favorites");

        let items = service.all().await.expect("list merged favorites");
        assert_eq!(items.len(), 2);
        assert_eq!(
            items
                .iter()
                .find(|item| item.comic_id == "1")
                .expect("local item")
                .title,
            "Local"
        );

        service
            .replace_all(&[super::FavoriteItem {
                comic_id: "3".into(),
                title: "Replacement".into(),
                author: "Remote".into(),
                description: String::new(),
                image: String::new(),
                tags: Vec::new(),
                favorited_at: original.favorited_at,
            }])
            .await
            .expect("replace favorites");
        let items = service.all().await.expect("list replacement");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].comic_id, "3");
    }

    #[tokio::test]
    async fn reads_requested_favorites_in_the_caller_order() {
        let service = test_service().await;
        for id in ["1", "2", "3"] {
            service
                .upsert(id, test_input(&format!("Favorite {id}")))
                .await
                .expect("seed favorite");
        }

        assert_eq!(service.count().await.expect("count favorites"), 3);
        let items = service
            .by_ids_in_order(&["3".into(), "1".into(), "missing".into(), "2".into()])
            .await
            .expect("read ordered favorites");
        assert_eq!(
            items
                .into_iter()
                .map(|item| item.comic_id)
                .collect::<Vec<_>>(),
            ["3", "1", "2"]
        );
        assert!(service
            .by_ids_in_order(&[])
            .await
            .expect("read empty id list")
            .is_empty());
    }

    async fn test_service() -> FavoriteService {
        let db = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await
            .expect("connect in-memory sqlite");
        sqlx::migrate!("./migrations")
            .run(&db)
            .await
            .expect("run migrations");
        FavoriteService::new(db)
    }

    fn test_input(title: &str) -> FavoriteInput {
        FavoriteInput {
            title: title.into(),
            author: "Author".into(),
            description: "Description".into(),
            image: "cover.jpg".into(),
            tags: vec!["Tag".into()],
        }
    }
}
