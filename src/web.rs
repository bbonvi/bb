use std::sync::Arc;

use axum::{
    extract::{self, Query, State},
    routing::{get, put},
    Router,
};

use crate::{
    app::App,
    bookmarks::{Bookmark, BookmarkShallow, Query as SearchQuery},
};

#[derive(Clone)]
struct SharedState {
    app: App,
}

async fn start_app(app: App) {
    let shared_state = Arc::new(SharedState { app });

    let app = Router::new()
        .route("/api/bookmarks", get(list_bookmarks))
        .route("/api/bookmarks", put(create_bookmark))
        .with_state(shared_state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await.unwrap();
    println!("listening on 0.0.0.0:8080");
    axum::serve(listener, app).await.unwrap();
}

// basic handler that responds with a static string
async fn list_bookmarks(
    State(state): State<Arc<SharedState>>,
    Query(search_query): Query<SearchQuery>,
) -> axum::Json<Vec<Bookmark>> {
    state.app.search(search_query).into()
}

// basic handler that responds with a static string
async fn create_bookmark(
    State(state): State<Arc<SharedState>>,
    extract::Json(shallow_bookmark): extract::Json<BookmarkShallow>,
) -> axum::Json<Bookmark> {
    tokio::task::block_in_place(move || {
        state
            .app
            .add(shallow_bookmark, false, false, false)
            .unwrap()
            .into()
    })
}

pub fn start_daemon(app: App) {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async { start_app(app).await });
}
