use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    routing::{delete, get, post, put},
    Json, Router,
};
use tokio::sync::RwLock;

use crate::{
    app::App,
    bookmarks::{Bookmark, BookmarkShallow, Query as SearchQuery},
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

async fn list_bookmarks(
    State(state): State<Arc<SharedState>>,
    Query(search_query): Query<SearchQuery>,
) -> axum::Json<Vec<Bookmark>> {
    let app = state.app.read().await;
    tokio::task::block_in_place(move || app.search(search_query).into())
}

async fn create_bookmark(
    State(state): State<Arc<SharedState>>,
    Json(shallow_bookmark): Json<BookmarkShallow>,
) -> axum::Json<Bookmark> {
    let mut app = state.app.write().await;
    tokio::task::block_in_place(move || {
        app.add(shallow_bookmark, false, false, false)
            .unwrap()
            .into()
    })
}

async fn update_bookmark(
    State(state): State<Arc<SharedState>>,
    Path(bookmark_id): Path<u64>,
    Json(shallow_bookmark): Json<BookmarkShallow>,
) -> axum::Json<Bookmark> {
    let mut app = state.app.write().await;
    tokio::task::block_in_place(move || app.update(bookmark_id, shallow_bookmark).unwrap().into())
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
