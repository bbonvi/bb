use crate::{
    app::{AppBackend, AppError, AppLocal, FetchMetadataOpts},
    bookmarks::{Bookmark, BookmarkUpdate, SearchQuery},
    config::Config,
    metadata::MetaOptions,
    parse_tags,
};
use anyhow::anyhow;
use axum::{extract::State, response::IntoResponse, routing::get, routing::post, Json, Router};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;
use tokio::{signal, sync::RwLock};

#[derive(Clone)]
struct SharedState {
    app: Arc<RwLock<AppLocal>>,
}

async fn start_app(app: AppLocal) {
    let app = Arc::new(RwLock::new(app));

    let signal = shutdown_signal(app.clone());
    let shared_state = Arc::new(SharedState { app: app.clone() });

    async fn shutdown_signal(app: Arc<RwLock<AppLocal>>) {
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
                    log::warn!("waiting for queues to stop");
                    app.wait_task_queue_finish();
                    break;
                }
            },
            _ = terminate => {},
        }
    }

    let app = Router::new()
        .nest_service("/api/file/", tower_http::services::ServeDir::new("uploads"))
        .route("/api/bookmarks/search", post(search))
        .route("/api/bookmarks/refresh_metadata", post(refresh_metadata))
        .route("/api/bookmarks/create", post(create))
        .route("/api/bookmarks/update", post(update))
        .route("/api/bookmarks/delete", post(delete))
        .route("/api/bookmarks/search_update", post(search_update))
        .route("/api/bookmarks/search_delete", post(search_delete))
        .route("/api/bookmarks/total", post(total))
        .route("/api/bookmarks/tags", post(tags))
        .route("/api/config", get(get_config))
        .route("/api/config", post(update_config))
        .layer(
            tower_http::trace::TraceLayer::new_for_http()
                .make_span_with(
                    tower_http::trace::DefaultMakeSpan::new().level(tracing::Level::INFO),
                )
                .on_response(
                    tower_http::trace::DefaultOnResponse::new().level(tracing::Level::INFO),
                ),
        )
        .with_state(shared_state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await.unwrap();
    log::info!("listening on 0.0.0.0:8080");
    axum::serve(listener, app)
        .with_graceful_shutdown(signal)
        .await
        .unwrap();
}

pub fn start_daemon(app: AppLocal) {
    tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap()
        .block_on(async { start_app(app).await });
}

// Make our own error that wraps `anyhow::Error`.
#[derive(Debug)]
struct HttpError(AppError);

// Tell axum how to convert `AppError` into a response.
impl IntoResponse for HttpError {
    fn into_response(self) -> axum::response::Response {
        match self.0 {
            AppError::NotFound => (
                axum::http::StatusCode::NOT_FOUND,
                json!({"error": self.0.to_string()}).to_string(),
            ),
            AppError::AlreadyExists(_) => (
                axum::http::StatusCode::CONFLICT,
                json!({"error": self.0.to_string()}).to_string(),
            ),
            AppError::Reqwest(_) => {
                log::error!("{self:?}");
                (
                    axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                    json!({"error": self.0.to_string()}).to_string(),
                )
            }
            AppError::IO(_) => {
                log::error!("{self:?}");
                (
                    axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                    json!({"error": self.0.to_string()}).to_string(),
                )
            }
            AppError::Other(_) => {
                log::error!("{self:?}");
                (
                    axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                    json!({"error": self.0.to_string()}).to_string(),
                )
            }
        }
        .into_response()
    }
}

// This enables using `?` on functions that return `Result<_, anyhow::Error>` to turn them into
// `Result<_, AppError>`. That way you don't need to do that manually.
impl<E> From<E> for HttpError
where
    E: Into<AppError>,
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
    /// *Exact search is turned off by default*
    #[serde(default)]
    pub exact: bool,

    #[serde(default)]
    pub descending: bool,
}

async fn search(
    State(state): State<Arc<SharedState>>,
    Json(payload): Json<ListBookmarksRequest>,
) -> Result<axum::Json<Vec<Bookmark>>, HttpError> {
    let app = state.app.clone();

    log::debug!("payload: {payload:?}");

    let search_query = SearchQuery {
        id: payload.id,
        title: payload.title,
        url: payload.url,
        description: payload.description,
        tags: payload.tags.map(|tags| parse_tags(tags)),
        exact: payload.exact,
        limit: None,
    };

    tokio::task::block_in_place(move || {
        let app = app.blocking_read();
        app.lazy_refresh_backend()?;

        app.search(search_query)
            .map(|mut bookmarks| {
                if payload.descending {
                    bookmarks.reverse();
                }
                bookmarks
            })
            .map(Into::into)
            .map_err(Into::into)
    })
}

#[derive(Deserialize, Serialize, Debug)]
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
}

async fn create(
    State(state): State<Arc<SharedState>>,
    Json(payload): Json<BookmarkCreateRequest>,
) -> Result<axum::Json<Bookmark>, HttpError> {
    log::debug!("payload: {payload:?}");

    let meta_opts = {
        if payload.no_meta {
            None
        } else {
            Some(MetaOptions {
                no_headless: payload.no_headless,
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
        app.lazy_refresh_backend()?;
        app.create(bmark_create, opts)
            .map(Into::into)
            .map_err(Into::into)
    })
}

#[derive(Deserialize, Serialize, Debug)]
pub struct BookmarkUpdateRequest {
    pub id: u64,
    pub title: Option<String>,
    pub description: Option<String>,
    pub tags: Option<String>,
    pub append_tags: Option<String>,
    pub remove_tags: Option<String>,
    pub url: Option<String>,
}

async fn update(
    State(state): State<Arc<SharedState>>,
    Json(payload): Json<BookmarkUpdateRequest>,
) -> Result<axum::Json<Bookmark>, HttpError> {
    log::debug!("payload: {payload:?}");

    let app = state.app.clone();

    let bmark_update = BookmarkUpdate {
        title: payload.title,
        description: payload.description,
        tags: payload.tags.map(parse_tags),
        remove_tags: payload.remove_tags.map(parse_tags),
        append_tags: payload.append_tags.map(parse_tags),
        url: payload.url,
        ..Default::default()
    };

    tokio::task::block_in_place(move || {
        let app = app.blocking_read();
        app.lazy_refresh_backend()?;
        app.update(payload.id, bmark_update)
            .map(Into::into)
            .map_err(Into::into)
    })
}

#[derive(Deserialize, Serialize)]
pub struct BookmarkDeleteRequest {
    pub id: u64,
}

async fn delete(
    State(state): State<Arc<SharedState>>,
    Json(payload): Json<BookmarkUpdateRequest>,
) -> Result<(), HttpError> {
    log::debug!("payload: {payload:?}");

    let app = state.app.clone();

    tokio::task::block_in_place(move || {
        let app = app.blocking_read();
        app.lazy_refresh_backend()?;
        app.delete(payload.id).map(|_| ()).map_err(Into::into)
    })
}

async fn search_delete(
    State(state): State<Arc<SharedState>>,
    Json(payload): Json<SearchQuery>,
) -> Result<axum::Json<usize>, HttpError> {
    log::debug!("payload: {payload:?}");

    let app = state.app.clone();

    tokio::task::block_in_place(move || {
        let app = app.blocking_read();
        app.bmark_mgr
            .search_delete(payload)
            .map(Into::into)
            .map_err(Into::into)
    })
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct SearchUpdateRequest {
    query: SearchQuery,
    update: BookmarkUpdate,
}

async fn search_update(
    State(state): State<Arc<SharedState>>,
    Json(payload): Json<SearchUpdateRequest>,
) -> Result<axum::Json<usize>, HttpError> {
    log::debug!("payload: {payload:?}");

    let app = state.app.clone();

    tokio::task::block_in_place(move || {
        let app = app.blocking_read();
        app.search_update(payload.query, payload.update)
            .map(Into::into)
            .map_err(Into::into)
    })
}

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct FetchMetaRequest {
    url: String,
    opts: FetchMetadataOpts,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct RefreshMetadataRequest {
    pub id: u64,

    /// Fetch metadata in background.
    ///
    /// *A bookmark will be added instantly*
    #[serde(default)]
    pub async_meta: bool,

    /// Do not use headless browser for metadata scrape
    #[serde(default)]
    pub no_headless: bool,
}

async fn refresh_metadata(
    State(state): State<Arc<SharedState>>,
    Json(payload): Json<RefreshMetadataRequest>,
) -> Result<axum::Json<()>, HttpError> {
    log::debug!("payload: {payload:?}");

    let app = state.app.clone();

    tokio::task::block_in_place(move || {
        let app = app.blocking_read();
        app.refresh_metadata(
            payload.id,
            crate::app::RefreshMetadataOpts {
                async_meta: payload.async_meta,
                meta_opts: MetaOptions {
                    no_headless: payload.no_headless,
                },
            },
        )
        .map(Into::into)
        .map_err(Into::into)
    })
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct TotalResponse {
    pub total: usize,
}
async fn total(
    State(state): State<Arc<SharedState>>,
) -> Result<axum::Json<TotalResponse>, HttpError> {
    let app = state.app.clone();

    tokio::task::block_in_place(move || {
        let app = app.blocking_read();
        app.total()
            .map(|total| TotalResponse { total }.into())
            .map_err(Into::into)
    })
}

async fn tags(State(state): State<Arc<SharedState>>) -> Result<axum::Json<Vec<String>>, HttpError> {
    let app = state.app.clone();

    tokio::task::block_in_place(move || {
        let app = app.blocking_read();
        app.tags().map(Into::into).map_err(Into::into)
    })
}

async fn get_config(
    State(state): State<Arc<SharedState>>,
) -> Result<axum::Json<Config>, HttpError> {
    let app = state.app.clone();

    tokio::task::block_in_place(move || {
        let app = app.blocking_read();
        Ok(app.config().read().unwrap().clone().into())
    })
}

async fn update_config(
    State(state): State<Arc<SharedState>>,
    Json(payload): Json<Config>,
) -> Result<axum::Json<Config>, HttpError> {
    let app = state.app.clone();

    tokio::task::block_in_place(move || {
        let app = app.blocking_read();
        *app.config().write().unwrap() = payload.clone();
        Ok(app.config().read().unwrap().clone().into())
    })
}
