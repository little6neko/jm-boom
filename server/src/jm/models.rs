use super::serde_ext::{
    bool_from_any, lossy_string_vec_from_array_or_scalar, optional_positive_i64_from_any,
    optional_string_from_any, string_from_any, string_from_any_or_default, u32_from_any,
};
use serde::Deserialize;

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "lowercase")]
pub(crate) enum FavoriteOrder {
    #[default]
    Mr,
    Mp,
}

impl FavoriteOrder {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Mr => "mr",
            Self::Mp => "mp",
        }
    }
}

// ============ Normalized JM Models ============

/// Comic basic info (used in lists, search results)
#[derive(Debug, Clone)]
pub struct Comic {
    pub id: String,
    pub name: String,
    pub author: String,
    pub description: String,
    pub image: String,
    pub tags: Vec<String>,
}

/// Search result
#[derive(Debug)]
pub struct SearchResult {
    pub total: u32,
    pub content: Vec<Comic>,
    pub redirect_aid: Option<String>,
}

/// Home feed section
#[derive(Debug)]
pub struct HomeSection {
    pub id: String,
    pub title: String,
    pub slug: String,
    pub section_type: String,
    pub filter_val: String,
    pub content: Vec<Comic>,
}

#[derive(Debug, Clone)]
pub struct FavoriteComic {
    pub id: String,
    pub name: String,
    pub author: String,
    pub description: String,
    pub image: String,
    pub tags: Vec<String>,
}

#[derive(Debug)]
pub struct FavoritePage {
    pub total: u32,
    pub items: Vec<FavoriteComic>,
}

// ============ Internal Payload Models ============

#[derive(Debug, Deserialize)]
pub(crate) struct SearchPayload {
    #[serde(default, deserialize_with = "u32_from_any")]
    pub total: u32,
    #[serde(default, deserialize_with = "optional_string_from_any")]
    pub redirect_aid: Option<String>,
    #[serde(default)]
    pub content: Vec<ComicPayload>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ComicPayload {
    #[serde(default, deserialize_with = "string_from_any_or_default")]
    pub id: String,
    #[serde(default, deserialize_with = "string_from_any_or_default")]
    pub name: String,
    #[serde(default, deserialize_with = "string_from_any_or_default")]
    pub author: String,
    #[serde(default, deserialize_with = "string_from_any_or_default")]
    pub description: String,
    #[serde(default, deserialize_with = "string_from_any_or_default")]
    pub image: String,
    #[serde(default, deserialize_with = "lossy_string_vec_from_array_or_scalar")]
    pub tags: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct FavoriteListPayload {
    #[serde(default, deserialize_with = "u32_from_any")]
    pub total: u32,
    #[serde(default)]
    pub list: Vec<FavoriteComicPayload>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct FavoriteComicPayload {
    #[serde(
        default,
        alias = "aid",
        alias = "AID",
        deserialize_with = "string_from_any_or_default"
    )]
    pub id: String,
    #[serde(default, deserialize_with = "string_from_any_or_default")]
    pub name: String,
    #[serde(default, deserialize_with = "string_from_any_or_default")]
    pub author: String,
    #[serde(default, deserialize_with = "string_from_any_or_default")]
    pub description: String,
    #[serde(default, deserialize_with = "string_from_any_or_default")]
    pub image: String,
    #[serde(default)]
    pub category: Option<FavoriteCategoryPayload>,
    #[serde(default)]
    pub category_sub: Option<FavoriteCategoryPayload>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct FavoriteCategoryPayload {
    #[serde(default, deserialize_with = "string_from_any_or_default")]
    pub title: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ComicFavoriteStatePayload {
    #[serde(default, deserialize_with = "bool_from_any")]
    pub is_favorite: bool,
}

#[derive(Debug, Deserialize)]
pub(crate) struct HomeSectionPayload {
    #[serde(deserialize_with = "string_from_any")]
    pub id: String,
    pub title: String,
    pub slug: String,
    #[serde(rename = "type")]
    pub section_type: String,
    #[serde(deserialize_with = "string_from_any")]
    pub filter_val: String,
    pub content: Vec<ComicPayload>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ComicDetailPayload {
    #[serde(deserialize_with = "string_from_any")]
    pub id: String,
    #[serde(default, deserialize_with = "string_from_any_or_default")]
    pub series_id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub image: String,
    #[serde(default, deserialize_with = "optional_positive_i64_from_any")]
    pub addtime: Option<i64>,
    #[serde(default, deserialize_with = "lossy_string_vec_from_array_or_scalar")]
    pub author: Vec<String>,
    #[serde(default, deserialize_with = "lossy_string_vec_from_array_or_scalar")]
    pub tags: Vec<String>,
    #[serde(default, deserialize_with = "lossy_string_vec_from_array_or_scalar")]
    pub actors: Vec<String>,
    #[serde(default, deserialize_with = "lossy_string_vec_from_array_or_scalar")]
    pub works: Vec<String>,
    #[serde(default, deserialize_with = "u32_from_any")]
    pub total_views: u32,
    #[serde(default, deserialize_with = "u32_from_any")]
    pub likes: u32,
    #[serde(default, deserialize_with = "u32_from_any")]
    pub comment_total: u32,
    #[serde(default)]
    pub related_list: Vec<RelatedComicPayload>,
    #[serde(default)]
    pub series: Vec<ChapterPayload>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct RelatedComicPayload {
    #[serde(deserialize_with = "string_from_any")]
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub image: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct ChapterPayload {
    #[serde(deserialize_with = "string_from_any")]
    pub id: String,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub sort: String,
}

// ============ Conversion Helpers ============

impl From<ComicPayload> for Comic {
    fn from(p: ComicPayload) -> Self {
        Self {
            id: p.id,
            name: p.name,
            author: p.author,
            description: p.description,
            image: p.image,
            tags: p.tags,
        }
    }
}

impl From<FavoriteComicPayload> for FavoriteComic {
    fn from(payload: FavoriteComicPayload) -> Self {
        let mut tags = Vec::new();
        for title in [payload.category, payload.category_sub]
            .into_iter()
            .flatten()
            .map(|category| category.title.trim().to_string())
        {
            if !title.is_empty() && !tags.contains(&title) {
                tags.push(title);
            }
        }

        Self {
            id: payload.id,
            name: payload.name,
            author: payload.author,
            description: payload.description,
            image: payload.image,
            tags,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ComicDetailPayload, ComicPayload, FavoriteListPayload};

    #[test]
    fn deserializes_mixed_scalar_and_array_fields_from_upstream_samples() {
        let comic: ComicPayload = serde_json::from_value(serde_json::json!({
            "id": 12345,
            "name": true,
            "author": 678,
            "description": null,
            "image": false,
            "tags": ["tag-a", 2, true, null, ""],
            "likes": "42",
            "total_views": 99
        }))
        .expect("decode mixed comic payload");
        assert_eq!(comic.id, "12345");
        assert_eq!(comic.name, "true");
        assert_eq!(comic.author, "678");
        assert_eq!(comic.description, "");
        assert_eq!(comic.image, "false");
        assert_eq!(comic.tags, vec!["tag-a", "2", "true"]);

        let detail: ComicDetailPayload = serde_json::from_value(serde_json::json!({
            "id": 54321,
            "series_id": 1423951,
            "name": "detail",
            "addtime": "1786371000",
            "author": "single-author",
            "tags": 7,
            "actors": ["actor-a", 8, false],
            "works": null,
            "total_views": "1001",
            "likes": true,
            "comment_total": null,
            "related_list": [{"id": 9, "name": "related"}],
            "series": [{
                "id": 10,
                "name": "chapter",
                "sort": "1"
            }]
        }))
        .expect("decode mixed comic detail payload");
        assert_eq!(detail.id, "54321");
        assert_eq!(detail.series_id, "1423951");
        assert_eq!(detail.author, vec!["single-author"]);
        assert_eq!(detail.tags, vec!["7"]);
        assert_eq!(detail.actors, vec!["actor-a", "8", "false"]);
        assert!(detail.works.is_empty());
        assert_eq!(detail.total_views, 1001);
        assert_eq!(detail.likes, 1);
        assert_eq!(detail.comment_total, 0);
        assert_eq!(detail.addtime, Some(1_786_371_000));
        assert_eq!(detail.related_list[0].id, "9");
        assert_eq!(detail.series[0].id, "10");
    }

    #[test]
    fn deserializes_remote_favorites_and_mixed_favorite_state() {
        let payload: FavoriteListPayload = serde_json::from_value(serde_json::json!({
            "total": "2",
            "list": [{
                "AID": 1455765,
                "name": "Example",
                "author": 42,
                "description": null,
                "image": false,
                "category": { "title": "Category" },
                "category_sub": { "title": "Category" }
            }, {
                "aid": "99",
                "name": true,
                "category_sub": { "title": 7 }
            }]
        }))
        .expect("decode favorite list");
        assert_eq!(payload.total, 2);

        let items = payload
            .list
            .into_iter()
            .map(super::FavoriteComic::from)
            .collect::<Vec<_>>();
        assert_eq!(items[0].id, "1455765");
        assert_eq!(items[0].author, "42");
        assert_eq!(items[0].description, "");
        assert_eq!(items[0].tags, vec!["Category"]);
        assert_eq!(items[1].id, "99");
        assert_eq!(items[1].tags, vec!["7"]);

        let state: super::ComicFavoriteStatePayload =
            serde_json::from_value(serde_json::json!({ "is_favorite": "1" }))
                .expect("decode favorite state");
        assert!(state.is_favorite);
    }

    #[test]
    fn ignores_invalid_or_non_positive_detail_timestamps() {
        for addtime in [
            serde_json::Value::Null,
            serde_json::json!(0),
            serde_json::json!(-1),
            serde_json::json!("not-a-timestamp"),
            serde_json::json!(true),
        ] {
            let detail: ComicDetailPayload = serde_json::from_value(serde_json::json!({
                "id": "1",
                "name": "detail",
                "addtime": addtime,
                "series": [{ "id": "2" }]
            }))
            .expect("decode invalid timestamps as missing");
            assert_eq!(detail.addtime, None);
        }
    }
}
