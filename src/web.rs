use crate::{
    app::App,
    bookmarks::{Bookmark, BookmarkUpdate, SearchQuery},
    metadata::MetaOptions,
    parse_tags,
};
use axum::{
    extract::{Path, Query, State},
    routing::{delete, get, post, put},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::{signal, sync::RwLock};

#[derive(Clone)]
struct SharedState {
    app: Arc<RwLock<App>>,
}

async fn start_app(app: App) {
    let app = Arc::new(RwLock::new(app));

    let signal = shutdown_signal(app.clone());
    let shared_state = Arc::new(SharedState { app: app.clone() });

    async fn shutdown_signal(app: Arc<RwLock<App>>) {
        let ctrl_c = async {
            signal::ctrl_c()
                .await
                .expect("failed to install Ctrl+C handler");
        };

        let terminate = async {
            signal::unix::signal(signal::unix::SignalKind::terminate())
                .expect("failed to install signal handler")
                .recv()
                .await;
        };

        tokio::select! {
            _ = ctrl_c => {
                let mut app = app.write().await;
                loop {
                    match app.metadata_queue.try_write() {
                        Ok(mut metadata_queue) => {
                            println!("signaling to stop queues");
                            // signals to stop accepting queues.
                            metadata_queue.push_front(None);
                        }
                        Err(_) => {
                            continue;
                        }
                    };

                    // join on queue thread handle
                    println!("waiting for queues to stop");
                    app.queue_handle.take().unwrap().join().unwrap();
                    break;
                }
            },
            _ = terminate => {},
        }
    }

    let app = Router::new()
        .route("/api/bookmarks", get(list_bookmarks))
        .route("/api/bookmarks", put(create_bookmark))
        .route("/api/bookmarks/:bookmark_id", post(update_bookmark))
        .route("/api/bookmarks/:bookmark_id", delete(delete_bookmark))
        .with_state(shared_state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await.unwrap();
    println!("listening on 0.0.0.0:8080");
    axum::serve(listener, app)
        .with_graceful_shutdown(signal)
        .await
        .unwrap();
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ListBookmarksRequest {
    pub id: Option<u64>,
    pub title: Option<String>,
    pub url: Option<String>,
    pub description: Option<String>,
    pub tags: Option<String>,

    /// Perform exact search.
    ///
    /// *Exact search on all fields except url is turned off by default*
    #[serde(default)]
    pub exact: bool,

    /// Do not perform exact search on url.
    ///
    /// *Exact search on url is on by default*
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

    /// Fetch metadata in background.
    ///
    /// *A bookmark will be added instantly*
    #[serde(default)]
    pub async_meta: bool,

    /// Do not fetch metadata.
    ///
    /// *A bookmark will be added instantly*
    #[serde(default)]
    pub no_meta: bool,

    /// Do not use headless browser for metadata scrape
    #[serde(default)]
    pub no_headless: bool,

    /// Do not use duckduckgo for metadata scrape
    #[serde(default)]
    pub no_duck: bool,
}

async fn create_bookmark(
    State(state): State<Arc<SharedState>>,
    Json(payload): Json<BookmarkCreateRequest>,
) -> axum::Json<Bookmark> {
    let meta_opts = {
        if payload.no_meta {
            None
        } else {
            Some(MetaOptions {
                no_headless: payload.no_headless,
                no_duck: payload.no_duck,
            })
        }
    };

    let opts = crate::app::AddOpts {
        no_https_upgrade: false,
        async_meta: payload.async_meta,
        meta_opts,
    };

    let bmark_create = crate::bookmarks::BookmarkCreate {
        title: payload.title,
        description: payload.description,
        tags: payload.tags.map(|tags| parse_tags(tags)),
        url: payload.url,
        ..Default::default()
    };

    let app = state.app.clone();

    tokio::task::block_in_place(move || {
        let app = app.blocking_read();
        app.add(bmark_create, opts).unwrap().into()
    })
}

#[derive(Deserialize, Serialize)]
pub struct BookmarkUpdateRequest {
    pub title: Option<String>,
    pub description: Option<String>,
    pub tags: Option<String>,
    pub url: Option<String>,
    //
    // TODO:
    // /// dont fetch metadata
    // #[serde(default)]
    // pub no_meta: bool,
    //
    // /// dont use headless browser for metadata scrape
    // #[serde(default)]
    // pub no_headless: bool,
    //
    // /// dont use duckduckgo for metadata scrape
    // #[serde(default)]
    // pub no_duck: bool,
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
        ..Default::default()
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
