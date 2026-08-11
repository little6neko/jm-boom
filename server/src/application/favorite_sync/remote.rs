use crate::{
    endpoint::{request_once, request_with_failover, EndpointManager},
    jm::{FavoriteOrder, FavoritePage, JmClient, JmResult},
};
use std::{future::Future, pin::Pin, sync::Arc};

pub(crate) type RemoteFuture<'a, T> = Pin<Box<dyn Future<Output = JmResult<T>> + Send + 'a>>;

pub(crate) trait FavoriteRemote: Send + Sync {
    fn favorite_page(&self, page: u32, order: FavoriteOrder) -> RemoteFuture<'_, FavoritePage>;
    fn favorite_state<'a>(&'a self, comic_id: &'a str) -> RemoteFuture<'a, bool>;
    fn toggle_favorite<'a>(&'a self, comic_id: &'a str) -> RemoteFuture<'a, ()>;
}

#[derive(Clone)]
pub(crate) struct JmFavoriteRemote {
    jm: Arc<JmClient>,
    endpoints: Arc<EndpointManager>,
}

impl JmFavoriteRemote {
    pub(crate) fn new(jm: Arc<JmClient>, endpoints: Arc<EndpointManager>) -> Self {
        Self { jm, endpoints }
    }
}

impl FavoriteRemote for JmFavoriteRemote {
    fn favorite_page(&self, page: u32, order: FavoriteOrder) -> RemoteFuture<'_, FavoritePage> {
        Box::pin(async move {
            request_with_failover(&self.jm, &self.endpoints, move |client, endpoint| {
                Box::pin(client.get_favorite_page(endpoint, page, order))
            })
            .await
            .map(|(_, payload)| payload)
        })
    }

    fn favorite_state<'a>(&'a self, comic_id: &'a str) -> RemoteFuture<'a, bool> {
        let comic_id = comic_id.to_string();
        Box::pin(async move {
            request_with_failover(&self.jm, &self.endpoints, move |client, endpoint| {
                let comic_id = comic_id.clone();
                Box::pin(async move { client.is_comic_favorite(endpoint, &comic_id).await })
            })
            .await
            .map(|(_, favorited)| favorited)
        })
    }

    fn toggle_favorite<'a>(&'a self, comic_id: &'a str) -> RemoteFuture<'a, ()> {
        let comic_id = comic_id.to_string();
        Box::pin(async move {
            request_once(&self.jm, &self.endpoints, move |client, endpoint| {
                let comic_id = comic_id.clone();
                Box::pin(async move { client.toggle_comic_favorite(endpoint, &comic_id).await })
            })
            .await
            .map(|_| ())
        })
    }
}
