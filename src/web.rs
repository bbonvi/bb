use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    routing::{delete, get, post, put},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

use crate::{
    app::App,
    bookmarks::{Bookmark, BookmarkUpdate, SearchQuery},
    parse_tags,
};

#[derive(Clone)]
struct SharedState {
    app: Arc<RwLock<App>>,
}

async fn start_app(app: App) {
    let shared_state = Arc::new(SharedState {
        app: Arc::new(RwLock::new(app)),
    });

    let app = Router::new()
        .route("/api/bookmarks", get(list_bookmarks))
        .route("/api/bookmarks", put(create_bookmark))
        .route("/api/bookmarks/:bookmark_id", post(update_bookmark))
        .route("/api/bookmarks/:bookmark_id", delete(delete_bookmark))
        .with_state(shared_state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await.unwrap();
    println!("listening on 0.0.0.0:8080");
    axum::serve(listener, app).await.unwrap();
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ListBookmarksRequest {
    pub id: Option<u64>,
    pub title: Option<String>,
    pub url: Option<String>,
    pub description: Option<String>,
    pub tags: Option<String>,

    #[serde(default)]
    pub exact: bool,
    #[serde(default)]
    pub no_exact_url: bool,
}

async fn list_bookmarks(
    State(state): State<Arc<SharedState>>,
    Query(query): Query<ListBookmarksRequest>,
) -> axum::Json<Vec<Bookmark>> {
    let app = state.app.read().await;

    let search_query = SearchQuery {
        id: query.id,
        title: query.title,
        url: query.url,
        description: query.description,
        tags: query.tags.map(|tags| parse_tags(tags)),
        exact: query.exact,
        no_exact_url: query.no_exact_url,
    };

    tokio::task::block_in_place(move || app.search(search_query).into())
}

#[derive(Deserialize, Serialize)]
pub struct BookmarkCreateRequest {
    pub title: Option<String>,
    pub description: Option<String>,
    pub tags: Option<String>,
    pub url: String,

    #[serde(default)]
    pub no_meta: bool,
    #[serde(default)]
    pub no_headless: bool,
}

async fn create_bookmark(
    State(state): State<Arc<SharedState>>,
    Json(payload): Json<BookmarkCreateRequest>,
) -> axum::Json<Bookmark> {
    let mut app = state.app.write().await;

    let bmark_create = crate::bookmarks::BookmarkCreate {
        title: payload.title,
        description: payload.description,
        tags: payload.tags.map(|tags| parse_tags(tags)),
        url: payload.url,
    };

    tokio::task::block_in_place(move || {
        app.add(bmark_create, false, payload.no_headless, payload.no_meta)
            .unwrap()
            .into()
    })
}

#[derive(Deserialize, Serialize)]
pub struct BookmarkUpdateRequest {
    pub title: Option<String>,
    pub description: Option<String>,
    pub tags: Option<String>,
    pub url: Option<String>,

    #[serde(default)]
    pub no_meta: bool,
    #[serde(default)]
    pub no_headless: bool,
}

async fn update_bookmark(
    State(state): State<Arc<SharedState>>,
    Path(bookmark_id): Path<u64>,
    Json(payload): Json<BookmarkUpdateRequest>,
) -> axum::Json<Bookmark> {
    let mut app = state.app.write().await;

    let bmark_update = BookmarkUpdate {
        title: payload.title,
        description: payload.description,
        tags: payload.tags.map(|tags| parse_tags(tags)),
        url: payload.url,
    };

    tokio::task::block_in_place(move || app.update(bookmark_id, bmark_update).unwrap().into())
}

async fn delete_bookmark(
    State(state): State<Arc<SharedState>>,
    Path(bookmark_id): Path<u64>,
) -> axum::Json<bool> {
    let mut app = state.app.write().await;
    tokio::task::block_in_place(move || app.delete(bookmark_id).unwrap().into())
}

pub fn start_daemon(app: App) {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async { start_app(app).await });
}
