use crate::{
    app::App,
    bookmarks::{Bookmark, BookmarkUpdate, SearchQuery},
    metadata::MetaOptions,
    parse_tags,
};
use anyhow::{anyhow, Context};
use axum::{
    extract::{Path, Query, State},
    response::IntoResponse,
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
                    app.shutdown();

                    // join on queue thread handle
                    println!("waiting for queues to stop");
                    app.wait_task_queue_finish();
                    break;
                }
            },
            _ = terminate => {},
        }
    }

    let app = Router::new()
        .nest_service("/api/file/", tower_http::services::ServeDir::new("uploads"))
        .route("/api/bookmarks", get(list_bookmarks))
        .route("/api/bookmarks", put(create_bookmark))
        .route("/api/bookmarks/:bmark_id", post(update_bookmark))
        .route("/api/bookmarks/:bmark_id", delete(delete_bookmark))
        .with_state(shared_state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await.unwrap();
    println!("listening on 0.0.0.0:8080");
    axum::serve(listener, app)
        .with_graceful_shutdown(signal)
        .await
        .unwrap();
}

// Make our own error that wraps `anyhow::Error`.
struct AppError(anyhow::Error);

// Tell axum how to convert `AppError` into a response.
impl IntoResponse for AppError {
    fn into_response(self) -> axum::response::Response {
        (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            format!("{}", self.0),
        )
            .into_response()
    }
}

// This enables using `?` on functions that return `Result<_, anyhow::Error>` to turn them into
// `Result<_, AppError>`. That way you don't need to do that manually.
impl<E> From<E> for AppError
where
    E: Into<anyhow::Error>,
{
    fn from(err: E) -> Self {
        Self(err.into())
    }
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
) -> Result<axum::Json<Vec<Bookmark>>, AppError> {
    let app = state.app.clone();

    let search_query = SearchQuery {
        id: query.id,
        title: query.title,
        url: query.url,
        description: query.description,
        tags: query.tags.map(|tags| parse_tags(tags)),
        exact: query.exact,
        no_exact_url: query.no_exact_url,
    };

    tokio::task::block_in_place(move || {
        let app = app.blocking_read();
        app.refresh_backend()?;
        app.search(search_query).map(Into::into).map_err(Into::into)
    })
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
) -> Result<axum::Json<Bookmark>, AppError> {
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
        app.refresh_backend()?;
        app.create(bmark_create, opts)
            .map(Into::into)
            .map_err(Into::into)
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
    Path(bmark_id): Path<u64>,
    Json(payload): Json<BookmarkUpdateRequest>,
) -> Result<axum::Json<Bookmark>, AppError> {
    let app = state.app.clone();

    let bmark_update = BookmarkUpdate {
        title: payload.title,
        description: payload.description,
        tags: payload.tags.map(|tags| parse_tags(tags)),
        url: payload.url,
        ..Default::default()
    };

    tokio::task::block_in_place(move || {
        let app = app.blocking_read();
        app.refresh_backend()?;
        app.update(bmark_id, bmark_update)?
            .ok_or_else(|| anyhow!("bookmark not found"))
            .map(Into::into)
            .map_err(Into::into)
    })
}

async fn delete_bookmark(
    State(state): State<Arc<SharedState>>,
    Path(bmark_id): Path<u64>,
) -> Result<axum::Json<bool>, AppError> {
    let app = state.app.clone();

    tokio::task::block_in_place(move || {
        let app = app.blocking_read();
        app.refresh_backend()?;
        app.delete(bmark_id)?
            .ok_or_else(|| anyhow!("bookmark not found"))
            .map(Into::into)
            .map_err(Into::into)
    })
}

pub fn start_daemon(app: App) {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async { start_app(app).await });
}
