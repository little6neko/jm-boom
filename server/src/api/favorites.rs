use super::{media::cover_url, COLLECTION_PAGE_SIZE};
use crate::{application::FavoriteInput, http_error::HttpError, jm::FavoriteOrder, AppState};
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize)]
pub(crate) struct FavoriteListQuery {
    #[serde(default = "default_page")]
    page: u32,
    #[serde(default)]
    order: FavoriteOrder,
}

fn default_page() -> u32 {
    1
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FavoriteListResponse {
    items: Vec<FavoriteResponse>,
    total: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct FavoriteResponse {
    id: String,
    title: String,
    author: String,
    description: String,
    image: String,
    tags: Vec<String>,
    favorited_at: i64,
}

pub async fn list(
    State(app): State<AppState>,
    Query(query): Query<FavoriteListQuery>,
) -> Result<Json<FavoriteListResponse>, HttpError> {
    validate_page(query.page)?;
    favorite_list(&app, query.page, query.order).await.map(Json)
}

pub async fn upsert(
    State(app): State<AppState>,
    Path(comic_id): Path<String>,
    Json(input): Json<FavoriteInput>,
) -> Result<Json<FavoriteResponse>, HttpError> {
    validate_comic_id(&comic_id)?;
    app.favorite_sync
        .upsert(&comic_id, input)
        .await
        .map(FavoriteResponse::from)
        .map(Json)
        .map_err(|error| error.into_http_error())
}

pub async fn remove(
    State(app): State<AppState>,
    Path(comic_id): Path<String>,
) -> Result<StatusCode, HttpError> {
    validate_comic_id(&comic_id)?;
    app.favorite_sync
        .remove(&comic_id)
        .await
        .map_err(|error| error.into_http_error())?;
    Ok(StatusCode::NO_CONTENT)
}

pub async fn clear(State(app): State<AppState>) -> Result<StatusCode, HttpError> {
    app.favorite_sync
        .clear()
        .await
        .map_err(|error| error.into_http_error())?;
    Ok(StatusCode::NO_CONTENT)
}

async fn favorite_list(
    app: &AppState,
    page: u32,
    order: FavoriteOrder,
) -> Result<FavoriteListResponse, HttpError> {
    let (items, total) = app
        .favorite_sync
        .list(page, COLLECTION_PAGE_SIZE, order)
        .await
        .map_err(|error| error.into_http_error())?;
    let items = items.into_iter().map(FavoriteResponse::from).collect();
    Ok(FavoriteListResponse { items, total })
}

impl From<crate::application::FavoriteItem> for FavoriteResponse {
    fn from(item: crate::application::FavoriteItem) -> Self {
        Self {
            image: cover_url(&item.comic_id, &item.image),
            id: item.comic_id,
            title: item.title,
            author: item.author,
            description: item.description,
            tags: item.tags,
            favorited_at: item.favorited_at,
        }
    }
}

fn validate_comic_id(comic_id: &str) -> Result<(), HttpError> {
    if comic_id.is_empty() || !comic_id.chars().all(|character| character.is_ascii_digit()) {
        return Err(HttpError::new(
            StatusCode::BAD_REQUEST,
            "收藏漫画 ID 必须为数字",
            false,
        ));
    }
    Ok(())
}

fn validate_page(page: u32) -> Result<(), HttpError> {
    if page == 0 {
        return Err(HttpError::new(
            StatusCode::BAD_REQUEST,
            "页码必须大于 0",
            false,
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{FavoriteListQuery, FavoriteResponse};
    use crate::{application::FavoriteItem, jm::FavoriteOrder};

    #[test]
    fn favorite_order_defaults_to_mr_and_rejects_unknown_values() {
        let default: FavoriteListQuery =
            serde_json::from_value(serde_json::json!({})).expect("deserialize default query");
        assert_eq!(default.page, 1);
        assert_eq!(default.order, FavoriteOrder::Mr);

        let mp: FavoriteListQuery = serde_json::from_value(serde_json::json!({
            "page": 2,
            "order": "mp"
        }))
        .expect("deserialize mp query");
        assert_eq!(mp.page, 2);
        assert_eq!(mp.order, FavoriteOrder::Mp);

        assert!(
            serde_json::from_value::<FavoriteListQuery>(serde_json::json!({
                "order": "unknown"
            }))
            .is_err()
        );
    }

    #[test]
    fn serializes_favorite_without_update_time() {
        let item = FavoriteItem {
            comic_id: "123".into(),
            title: "Example".into(),
            author: "Author".into(),
            description: String::new(),
            image: String::new(),
            tags: Vec::new(),
            favorited_at: 456,
        };
        let response = FavoriteResponse::from(item);

        let value = serde_json::to_value(response).expect("serialize favorite response");
        assert!(value.get("updatedAt").is_none());
    }
}
