use crate::{
    app::{AppBackend, AppError, AppLocal, FetchMetadataOpts},
    bookmarks::{self, Bookmark, BookmarkUpdate, SearchQuery},
    config::Config,
    images,
    metadata::MetaOptions,
    parse_tags,
    task_runner::{read_queue_dump, QueueDump},
};
use axum::{
    extract::{DefaultBodyLimit, State},
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use base64::{engine::general_purpose::STANDARD, Engine as _};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{fmt::Debug, sync::Arc};
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
        .route("/api/task_queue", get(task_queue))
        .layer(DefaultBodyLimit::max(100 * 1024 * 1024))
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
            AppError::Base64(_) => {
                log::error!("{self:?}");
                (
                    axum::http::StatusCode::BAD_REQUEST,
                    json!({"error": self.0.to_string()}).to_string(),
                )
            }
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

#[derive(Deserialize, Serialize)]
pub struct BookmarkCreateRequest {
    pub title: Option<String>,
    pub description: Option<String>,
    pub tags: Option<String>,
    pub url: String,

    pub image_b64: Option<String>,
    pub icon_b64: Option<String>,

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

impl Debug for BookmarkCreateRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "BookmarkCreateRequest {{ title: {:?}, description: {:?}, tags: {:?}, url: {:?}, image_b64: [REDUCTED], icon_b64: [REDUCTED], async_meta: {:?}, no_meta: {:?}, no_headless: {:?} }}", self.title, self.description, self.tags, self.url, self.async_meta, self.no_meta, self.no_headless)
    }
}

async fn create(
    State(state): State<Arc<SharedState>>,
    Json(payload): Json<BookmarkCreateRequest>,
) -> Result<axum::Json<Bookmark>, HttpError> {
    log::debug!("payload: {payload:?}");

    // we turn off metadata fetch requet by default to
    // upload custom files and then initiate fetch request

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
        meta_opts: None,
        skip_rules: true,
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

        let bmark = app.create(bmark_create, opts)?;

        if let Some(cover) = payload.image_b64 {
            let file = STANDARD
                .decode(cover)
                .map_err(|err| AppError::Other(err.into()))?;

            let file = images::convert(file, Some((800, 800)), true)?;
            app.upload_cover(bmark.id, file)?;
        }

        if let Some(icon) = payload.icon_b64 {
            let file = STANDARD
                .decode(icon)
                .map_err(|err| AppError::Other(err.into()))?;
            let file = images::convert(file, None, true)?;
            app.upload_icon(bmark.id, file)?;
        }

        if let Some(meta_opts) = meta_opts {
            let refresh_meta_opts = crate::app::RefreshMetadataOpts {
                async_meta: payload.async_meta,
                meta_opts,
            };
            app.refresh_metadata(bmark.id, refresh_meta_opts)?;
        };

        return Ok(app
            .search(bookmarks::SearchQuery {
                id: Some(bmark.id),
                ..Default::default()
            })?
            .first()
            .unwrap()
            .clone()
            .into());
    })
}

#[derive(Deserialize, Serialize)]
pub struct BookmarkUpdateRequest {
    pub id: u64,
    pub title: Option<String>,
    pub description: Option<String>,
    pub tags: Option<String>,
    pub append_tags: Option<String>,
    pub remove_tags: Option<String>,
    pub url: Option<String>,

    pub image_b64: Option<String>,
    pub icon_b64: Option<String>,
}

impl Debug for BookmarkUpdateRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "BookmarkUpdateRequest {{ id: {}, title: {:?}, description: {:?}, tags: {:?}, append_tags: {:?}, remove_tags: {:?}, url: {:?}, image_b64: [REDUCTED], icon_b64: [REDUCTED] }}", self.id, self.title, self.description, self.tags, self.append_tags, self.remove_tags, self.url)
    }
}

async fn update(
    State(state): State<Arc<SharedState>>,
    Json(payload): Json<BookmarkUpdateRequest>,
) -> Result<axum::Json<Bookmark>, HttpError> {
    log::debug!("payload: {payload:?}",);

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
        let mut bmark = app.update(payload.id, bmark_update)?;

        if let Some(cover) = payload.image_b64 {
            let file = STANDARD.decode(cover)?;
            let file = images::convert(file, Some((800, 800)), true)?;
            bmark = app.upload_cover(bmark.id, file)?;
        }

        if let Some(icon) = payload.icon_b64 {
            let file = STANDARD.decode(icon).map_err(|err| err)?;
            let file = images::convert(file, None, true)?;
            bmark = app.upload_icon(bmark.id, file)?;
        }

        Ok(bmark.into())
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

async fn task_queue() -> Result<axum::Json<QueueDump>, HttpError> {
    tokio::task::block_in_place(move || Ok(read_queue_dump().into()))
}
