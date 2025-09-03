use crate::{
    app::{
        backend::*,
        service::AppService,
        task_runner::{self, QueueDump},
    },
    bookmarks::{Bookmark, BookmarkCreate, BookmarkUpdate, SearchQuery},
    config::Config,
    eid::Eid,
    metadata::MetaOptions,
    storage::{self, StorageManager},
};
use anyhow::Context;
use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use base64::Engine;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, RwLock};
use tower_http::services::ServeDir;
use tokio::signal;

struct SharedState {
    app_service: Arc<RwLock<AppService>>,
    storage_mgr: Arc<dyn StorageManager>,
}

async fn start_app(app_service: AppService, base_path: &str) {
    let storage_mgr = Arc::new(storage::BackendLocal::new(&format!("{base_path}/uploads")));
    let shared_state = Arc::new(RwLock::new(SharedState {
        app_service: Arc::new(RwLock::new(app_service)),
        storage_mgr,
    }));

    let app = Router::new()
        .route("/api/bookmarks/search", post(search))
        .route("/api/bookmarks/create", post(create))
        .route("/api/bookmarks/update", post(update))
        .route("/api/bookmarks/delete", post(delete))
        .route("/api/bookmarks/search_delete", post(search_delete))
        .route("/api/bookmarks/search_update", post(search_update))
        .route("/api/bookmarks/refresh_metadata", post(refresh_metadata))
        .route("/api/bookmarks/total", post(total))
        .route("/api/bookmarks/tags", post(tags))
        .route("/api/config", get(get_config).post(update_config))
        .route("/api/task_queue", get(task_queue))
        .with_state(shared_state.clone())
        .nest_service("/", ServeDir::new(format!("{base_path}/web")));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await.unwrap();
    log::info!("listening on {}", listener.local_addr().unwrap());

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal(shared_state))
        .await
        .unwrap();
}

async fn shutdown_signal(_app_service: Arc<RwLock<SharedState>>) {
    // Wait for shutdown signals (Ctrl+C, SIGTERM, etc.)
    tokio::select! {
        _ = signal::ctrl_c() => {
            log::info!("Received Ctrl+C, shutting down web server");
        }
        _ = async {
            if let Ok(mut sig) = signal::unix::signal(signal::unix::SignalKind::terminate()) {
                sig.recv().await;
            }
        } => {
            log::info!("Received SIGTERM, shutting down web server");
        }
    }
}

pub fn start_daemon(app: crate::app::local::AppLocal, base_path: &str) {
    let app_service = AppService::new(Box::new(app));
    tokio::runtime::Runtime::new()
        .unwrap()
        .block_on(start_app(app_service, base_path));
}

#[derive(thiserror::Error, Debug)]
pub enum AppError {
    #[error("reqwest error: {0:?}")]
    Reqwest(#[from] reqwest::Error),

    #[error("io error: {0:?}")]
    IO(#[from] std::io::Error),

    #[error("Base64: {0:?}")]
    Base64(#[from] base64::DecodeError),

    #[error("unexpected error: {0:?}")]
    Other(#[from] anyhow::Error),
}

impl IntoResponse for AppError {
    fn into_response(self) -> axum::response::Response {
        let (status, error_message) = match self {
            AppError::Reqwest(_) => (StatusCode::BAD_GATEWAY, self.to_string()),
            AppError::IO(_) => (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()),
            AppError::Base64(_) => (StatusCode::BAD_REQUEST, self.to_string()),
            AppError::Other(_) => (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()),
        };

        let body = Json(serde_json::json!({
            "error": error_message
        }));

        (status, body).into_response()
    }
}

#[derive(Deserialize)]
pub struct ListBookmarksRequest {
    pub query: SearchQuery,
    pub limit: Option<usize>,
}

async fn search(
    State(state): State<Arc<RwLock<SharedState>>>,
    Json(payload): Json<ListBookmarksRequest>,
) -> Result<axum::Json<Vec<Bookmark>>, AppError> {
    let state = state.read().unwrap();
    let app_service = state.app_service.read().unwrap();

    let mut query = payload.query;
    query.limit = payload.limit;

    let bookmarks = app_service
        .search_bookmarks(query, false)
        .context("Failed to search bookmarks")?;

    Ok(axum::Json(bookmarks))
}

#[derive(Deserialize)]
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

impl std::fmt::Debug for BookmarkCreateRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BookmarkCreateRequest")
            .field("title", &self.title)
            .field("description", &self.description)
            .field("tags", &self.tags)
            .field("url", &self.url)
            .field(
                "image_b64",
                &self.image_b64.as_ref().map(|_| "[BASE64_DATA]"),
            )
            .field("icon_b64", &self.icon_b64.as_ref().map(|_| "[BASE64_DATA]"))
            .field("async_meta", &self.async_meta)
            .field("no_meta", &self.no_meta)
            .field("no_headless", &self.no_headless)
            .finish()
    }
}

async fn create(
    State(state): State<Arc<RwLock<SharedState>>>,
    Json(payload): Json<BookmarkCreateRequest>,
) -> Result<axum::Json<Bookmark>, AppError> {
    let state = state.read().unwrap();
    let app_service = state.app_service.read().unwrap();

    let mut create = BookmarkCreate {
        title: payload.title,
        description: payload.description,
        tags: payload.tags.map(|tags| {
            tags.split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect()
        }),
        url: payload.url,
        ..Default::default()
    };

    // Handle base64 image/icon uploads
    if let Some(image_b64) = payload.image_b64 {
        let image_data = base64::engine::general_purpose::STANDARD
            .decode(image_b64)
            .context("Failed to decode base64 image data")?;
        let image_id = format!("{}.png", Eid::new());
        state.storage_mgr.write(&image_id, &image_data);
        create.image_id = Some(image_id);
    }

    if let Some(icon_b64) = payload.icon_b64 {
        let icon_data = base64::engine::general_purpose::STANDARD
            .decode(icon_b64)
            .context("Failed to decode base64 icon data")?;
        let icon_id = format!("{}.png", Eid::new());
        state.storage_mgr.write(&icon_id, &icon_data);
        create.icon_id = Some(icon_id);
    }

    let add_opts = AddOpts {
        no_https_upgrade: false,
        async_meta: payload.async_meta,
        meta_opts: if payload.no_meta {
            None
        } else {
            Some(MetaOptions {
                no_headless: payload.no_headless,
            })
        },
        skip_rules: false,
    };

    let bookmark = app_service
        .create_bookmark(create, add_opts)
        .context("Failed to create bookmark")?;

    Ok(axum::Json(bookmark))
}

#[derive(Deserialize)]
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

impl std::fmt::Debug for BookmarkUpdateRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BookmarkUpdateRequest")
            .field("id", &self.id)
            .field("title", &self.title)
            .field("description", &self.description)
            .field("tags", &self.tags)
            .field("append_tags", &self.append_tags)
            .field("remove_tags", &self.remove_tags)
            .field("url", &self.url)
            .field(
                "image_b64",
                &self.image_b64.as_ref().map(|_| "[BASE64_DATA]"),
            )
            .field("icon_b64", &self.icon_b64.as_ref().map(|_| "[BASE64_DATA]"))
            .finish()
    }
}

async fn update(
    State(state): State<Arc<RwLock<SharedState>>>,
    Json(payload): Json<BookmarkUpdateRequest>,
) -> Result<axum::Json<Bookmark>, AppError> {
    let state = state.read().unwrap();
    let app_service = state.app_service.read().unwrap();

    let mut update = BookmarkUpdate {
        title: payload.title,
        description: payload.description,
        tags: payload.tags.map(|tags| {
            tags.split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect()
        }),
        append_tags: payload.append_tags.map(|tags| {
            tags.split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect()
        }),
        remove_tags: payload.remove_tags.map(|tags| {
            tags.split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect()
        }),
        url: payload.url,
        ..Default::default()
    };

    // Handle base64 image/icon uploads
    if let Some(image_b64) = payload.image_b64 {
        let image_data = base64::engine::general_purpose::STANDARD
            .decode(image_b64)
            .context("Failed to decode base64 image data")?;
        let image_id = format!("{}.png", Eid::new());
        state.storage_mgr.write(&image_id, &image_data);
        update.image_id = Some(image_id);
    }

    if let Some(icon_b64) = payload.icon_b64 {
        let icon_data = base64::engine::general_purpose::STANDARD
            .decode(icon_b64)
            .context("Failed to decode base64 icon data")?;
        let icon_id = format!("{}.png", Eid::new());
        state.storage_mgr.write(&icon_id, &icon_data);
        update.icon_id = Some(icon_id);
    }

    let bookmark = app_service
        .update_bookmark(payload.id, update)
        .context("Failed to update bookmark")?;

    Ok(axum::Json(bookmark))
}

#[derive(Deserialize)]
pub struct BookmarkDeleteRequest {
    pub _id: u64,
}

async fn delete(
    State(state): State<Arc<RwLock<SharedState>>>,
    Json(payload): Json<BookmarkUpdateRequest>,
) -> Result<(), AppError> {
    let state = state.read().unwrap();
    let app_service = state.app_service.read().unwrap();

    app_service
        .delete_bookmark(payload.id)
        .context("Failed to delete bookmark")?;

    Ok(())
}

async fn search_delete(
    State(state): State<Arc<RwLock<SharedState>>>,
    Json(payload): Json<SearchQuery>,
) -> Result<axum::Json<usize>, AppError> {
    let state = state.read().unwrap();
    let app_service = state.app_service.read().unwrap();

    let count = app_service
        .search_and_delete(payload)
        .context("Failed to delete bookmarks")?;

    Ok(axum::Json(count))
}

#[derive(Deserialize)]
pub struct SearchUpdateRequest {
    query: SearchQuery,
    update: BookmarkUpdate,
}

async fn search_update(
    State(state): State<Arc<RwLock<SharedState>>>,
    Json(payload): Json<SearchUpdateRequest>,
) -> Result<axum::Json<usize>, AppError> {
    let state = state.read().unwrap();
    let app_service = state.app_service.read().unwrap();

    let count = app_service
        .search_and_update(payload.query, payload.update)
        .context("Failed to update bookmarks")?;

    Ok(axum::Json(count))
}

#[derive(Deserialize)]
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
    State(state): State<Arc<RwLock<SharedState>>>,
    Json(payload): Json<RefreshMetadataRequest>,
) -> Result<axum::Json<()>, AppError> {
    let state = state.read().unwrap();
    let app_service = state.app_service.read().unwrap();

    let opts = RefreshMetadataOpts {
        async_meta: payload.async_meta,
        meta_opts: MetaOptions {
            no_headless: payload.no_headless,
        },
    };

    app_service
        .refresh_metadata(payload.id, opts)
        .context("Failed to refresh metadata")?;

    Ok(axum::Json(()))
}

#[derive(Serialize, Deserialize, Clone)]
pub struct TotalResponse {
    pub total: usize,
}

async fn total(
    State(state): State<Arc<RwLock<SharedState>>>,
) -> Result<axum::Json<TotalResponse>, AppError> {
    let state = state.read().unwrap();
    let app_service = state.app_service.read().unwrap();

    let total = app_service
        .get_total_count()
        .context("Failed to get total count")?;

    Ok(axum::Json(TotalResponse { total }))
}

async fn tags(
    State(state): State<Arc<RwLock<SharedState>>>,
) -> Result<axum::Json<Vec<String>>, AppError> {
    let state = state.read().unwrap();
    let app_service = state.app_service.read().unwrap();

    let tags = app_service.get_tags().context("Failed to get tags")?;

    Ok(axum::Json(tags))
}

async fn get_config(
    State(state): State<Arc<RwLock<SharedState>>>,
) -> Result<axum::Json<Config>, AppError> {
    let state = state.read().unwrap();
    let app_service = state.app_service.read().unwrap();

    let config = app_service.get_config().context("Failed to get config")?;

    let config_value = config.read().unwrap().clone();
    Ok(axum::Json(config_value))
}

async fn update_config(
    State(state): State<Arc<RwLock<SharedState>>>,
    Json(payload): Json<Config>,
) -> Result<axum::Json<Config>, AppError> {
    let state = state.read().unwrap();
    let app_service = state.app_service.read().unwrap();

    app_service
        .update_config(payload.clone())
        .context("Failed to update config")?;

    Ok(axum::Json(payload))
}

async fn task_queue() -> Result<axum::Json<QueueDump>, AppError> {
    let queue_dump = task_runner::read_queue_dump();
    Ok(axum::Json(queue_dump))
}
