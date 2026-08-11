use crate::{
    application::{FavoriteSyncResolution, FavoriteSyncState},
    http_error::HttpError,
    AppState,
};
use axum::{extract::State, Json};
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct FavoriteSyncToggle {
    enabled: bool,
}

#[derive(Debug, Deserialize)]
pub struct FavoriteSyncResolve {
    strategy: FavoriteSyncResolution,
}

pub async fn get(State(app): State<AppState>) -> Json<FavoriteSyncState> {
    Json(app.favorite_sync.state().await)
}

pub async fn set(
    State(app): State<AppState>,
    Json(payload): Json<FavoriteSyncToggle>,
) -> Result<Json<FavoriteSyncState>, HttpError> {
    app.favorite_sync
        .set_enabled(payload.enabled)
        .await
        .map(Json)
        .map_err(|error| error.into_http_error())
}

pub async fn check(State(app): State<AppState>) -> Result<Json<FavoriteSyncState>, HttpError> {
    app.favorite_sync
        .check()
        .await
        .map(Json)
        .map_err(|error| error.into_http_error())
}

pub async fn resolve(
    State(app): State<AppState>,
    Json(payload): Json<FavoriteSyncResolve>,
) -> Result<Json<FavoriteSyncState>, HttpError> {
    app.favorite_sync
        .resolve(payload.strategy)
        .await
        .map(Json)
        .map_err(|error| error.into_http_error())
}

pub async fn retry(State(app): State<AppState>) -> Result<Json<FavoriteSyncState>, HttpError> {
    app.favorite_sync
        .retry()
        .await
        .map(Json)
        .map_err(|error| error.into_http_error())
}
