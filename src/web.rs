use crate::{
    app::{
        backend::*,
        service::AppService,
        task_runner::{self, QueueDump},
    },
    auth::{AuthConfig, AuthLayer},
    bookmarks::{Bookmark, BookmarkCreate, BookmarkUpdate, SearchQuery},
    config::Config,
    eid::Eid,
    images,
    metadata::MetaOptions,
    storage::{self, StorageManager},
    workspaces::{WorkspaceError, WorkspaceStore},
};
use anyhow::Context;
use axum::{
    extract::{DefaultBodyLimit, Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete as delete_method, get, post, put},
    Json, Router,
};
use base64::Engine;
use serde::{Deserialize, Serialize};
use std::sync::{Arc, RwLock};
use tokio::signal;
use tower_http::services::{ServeDir, ServeFile};

struct SharedState {
    app_service: Arc<RwLock<AppService>>,
    storage_mgr: Arc<dyn StorageManager>,
    workspace_store: Arc<RwLock<WorkspaceStore>>,
}

async fn start_app(app_service: AppService, base_path: &str) {
    let storage_mgr = Arc::new(storage::BackendLocal::new(&format!("{base_path}/uploads")));
    let workspace_store = WorkspaceStore::load(base_path)
        .expect("failed to load workspace store");
    let shared_state = Arc::new(RwLock::new(SharedState {
        app_service: Arc::new(RwLock::new(app_service)),
        storage_mgr,
        workspace_store: Arc::new(RwLock::new(workspace_store)),
    }));

    let webui = Router::new()
        .nest_service("/", ServeFile::new("client/build/index.html"))
        .nest_service("/static/", ServeDir::new("client/build/static/"))
        .nest_service(
            "/asset-manifest.json",
            ServeFile::new("client/build/asset-manifest.json"),
        )
        .nest_service("/favicon.png", ServeFile::new("client/build/favicon.png"))
        .nest_service("/logo192.png", ServeFile::new("client/build/logo192.png"))
        .nest_service("/logo512.png", ServeFile::new("client/build/logo512.png"))
        .nest_service(
            "/manifest.json",
            ServeFile::new("client/build/manifest.json"),
        )
        .nest_service("/robots.txt", ServeFile::new("client/build/robots.txt"));

    let uploads_path = format!("{base_path}/uploads");

    // Load auth config from environment
    let auth_config = AuthConfig::from_env();
    let auth_layer = AuthLayer::new(auth_config);

    let uploads = Router::new()
        .nest_service("/api/file/", ServeDir::new(&uploads_path))
        .layer(auth_layer.clone());

    let api = Router::new()
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
        .route("/api/semantic/status", get(semantic_status))
        .route("/api/workspaces", get(list_workspaces))
        .route("/api/workspaces", post(create_workspace))
        .route("/api/workspaces/reorder", post(reorder_workspaces))
        .route("/api/workspaces/:id", put(update_workspace))
        .route("/api/workspaces/:id", delete_method(delete_workspace))
        .layer(auth_layer);

    // Health endpoint - no auth required (for container health checks)
    let health = Router::new().route("/api/health", get(health_check));

    let tracing_layer = tower_http::trace::TraceLayer::new_for_http()
        .make_span_with(tower_http::trace::DefaultMakeSpan::new().level(tracing::Level::INFO))
        .on_response(tower_http::trace::DefaultOnResponse::new().level(tracing::Level::INFO));

    let app = Router::new()
        .merge(webui)
        .merge(uploads)
        .merge(api)
        .merge(health)
        .layer(DefaultBodyLimit::max(100 * 1024 * 1024))
        .layer(tracing_layer)
        .with_state(shared_state.clone());

    let listener = tokio::net::TcpListener::bind("0.0.0.0:8080").await.unwrap();
    log::info!("listening on 0.0.0.0:8080");
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal(shared_state.clone()))
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

async fn health_check() -> impl IntoResponse {
    Json(serde_json::json!({"status": "ok"}))
}

pub fn start_daemon(app: crate::app::local::AppLocal, base_path: &str) {
    let config = app.config();
    let semantic_config = config.read().unwrap().semantic_search.clone();

    let app_service = if semantic_config.enabled {
        log::info!("Semantic search enabled, initializing service");
        let semantic_service = std::sync::Arc::new(
            crate::semantic::SemanticSearchService::new(
                semantic_config,
                std::path::PathBuf::from(base_path),
            )
        );
        AppService::with_semantic(Box::new(app), semantic_service)
    } else {
        log::info!("Semantic search disabled");
        AppService::new(Box::new(app))
    };

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

    #[error("{message}")]
    SemanticDisabled { message: String },

    #[error("{message}")]
    InvalidThreshold { message: String },

    #[error("{message}")]
    ModelUnavailable { message: String },

    #[error("{0}")]
    Workspace(#[from] WorkspaceError),

    #[error("unexpected error: {0:?}")]
    Other(#[from] anyhow::Error),
}

impl IntoResponse for AppError {
    fn into_response(self) -> axum::response::Response {
        let (status, code, message) = match &self {
            AppError::Reqwest(_) => (StatusCode::BAD_GATEWAY, "GATEWAY_ERROR", self.to_string()),
            AppError::IO(_) => (StatusCode::INTERNAL_SERVER_ERROR, "IO_ERROR", self.to_string()),
            AppError::Base64(_) => (StatusCode::BAD_REQUEST, "BASE64_ERROR", self.to_string()),
            AppError::SemanticDisabled { message } => {
                (StatusCode::UNPROCESSABLE_ENTITY, "SEMANTIC_DISABLED", message.clone())
            }
            AppError::InvalidThreshold { message } => {
                (StatusCode::BAD_REQUEST, "INVALID_THRESHOLD", message.clone())
            }
            AppError::ModelUnavailable { message } => {
                (StatusCode::SERVICE_UNAVAILABLE, "MODEL_UNAVAILABLE", message.clone())
            }
            AppError::Workspace(ref e) => {
                let status = match e {
                    WorkspaceError::NotFound(_) => StatusCode::NOT_FOUND,
                    WorkspaceError::InvalidName
                    | WorkspaceError::DuplicateName(_)
                    | WorkspaceError::InvalidPattern { .. }
                    | WorkspaceError::InvalidReorder(_) => StatusCode::BAD_REQUEST,
                    WorkspaceError::Storage(_) => StatusCode::INTERNAL_SERVER_ERROR,
                };
                (status, "WORKSPACE_ERROR", self.to_string())
            }
            AppError::Other(_) => {
                (StatusCode::INTERNAL_SERVER_ERROR, "INTERNAL_ERROR", self.to_string())
            }
        };

        let body = Json(serde_json::json!({
            "error": code,
            "message": message
        }));

        (status, body).into_response()
    }
}

#[derive(Deserialize, Debug)]
pub struct ListBookmarksRequest {
    pub id: Option<u64>,
    pub title: Option<String>,
    pub url: Option<String>,
    pub description: Option<String>,
    pub tags: Option<String>,
    pub keyword: Option<String>,

    /// Semantic search query text
    #[serde(default)]
    pub semantic: Option<String>,

    /// Similarity threshold for semantic search [0.0, 1.0]
    #[serde(default)]
    pub threshold: Option<f32>,

    #[serde(default)]
    pub exact: bool,

    #[serde(default)]
    pub limit: Option<usize>,
}

async fn search(
    State(state): State<Arc<RwLock<SharedState>>>,
    Json(payload): Json<ListBookmarksRequest>,
) -> Result<axum::Json<Vec<Bookmark>>, AppError> {
    log::info!("Search bookmarks request: {:?}", payload);

    // Validate threshold before processing
    if let Some(threshold) = payload.threshold {
        if !(0.0..=1.0).contains(&threshold) {
            return Err(AppError::InvalidThreshold {
                message: format!(
                    "Threshold must be between 0.0 and 1.0, got {}",
                    threshold
                ),
            });
        }
    }

    let state = state.read().unwrap();
    let app_service = state.app_service.read().unwrap();

    let query = SearchQuery {
        id: payload.id,
        title: payload.title,
        url: payload.url,
        description: payload.description,
        tags: payload.tags.map(crate::parse_tags),
        keyword: payload.keyword,
        semantic: payload.semantic,
        threshold: payload.threshold,
        exact: payload.exact,
        limit: payload.limit,
    };
    log::info!("Search bookmarks with query: {:?}", query);

    let bookmarks = app_service
        .search_bookmarks(query, false)
        .map_err(|e| {
            // Map semantic-disabled errors to proper HTTP response
            let err_msg = e.to_string();
            if err_msg.contains("Semantic search is disabled") {
                AppError::SemanticDisabled {
                    message: "Semantic search is disabled in configuration".to_string(),
                }
            } else if err_msg.contains("model") && err_msg.contains("unavailable")
                || err_msg.contains("Failed to initialize")
            {
                AppError::ModelUnavailable {
                    message: "Semantic search model is unavailable".to_string(),
                }
            } else {
                AppError::Other(e)
            }
        })?;

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
    log::info!("Create bookmark request: {:?}", create);

    // Handle base64 image/icon uploads
    if let Some(image_b64) = payload.image_b64 {
        let image_data = base64::engine::general_purpose::STANDARD
            .decode(image_b64)
            .context("Failed to decode base64 image data")?;

        // Compress preview image to WebP
        let config = app_service.get_config().context("Failed to get config")?;
        let img_config = &config.read().unwrap().images;
        let compressed = images::compress_image(&image_data, img_config.max_size, img_config.quality)
            .context("Failed to compress image")?;

        let image_id = format!("{}.webp", Eid::new());
        state.storage_mgr.write(&image_id, &compressed.data);
        create.image_id = Some(image_id);
    }

    // Icons stay as-is (favicons are typically small already)
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
    log::info!("Search and delete bookmarks with query: {:?}", payload);
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

/// Response for GET /api/semantic/status
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct SemanticStatusResponse {
    /// Whether semantic search is enabled in configuration
    pub enabled: bool,
    /// Model name used for embeddings
    pub model: String,
    /// Number of bookmarks with embeddings in the index
    pub indexed_count: usize,
    /// Total number of bookmarks in the database
    pub total_bookmarks: usize,
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

async fn semantic_status(
    State(state): State<Arc<RwLock<SharedState>>>,
) -> Result<axum::Json<SemanticStatusResponse>, AppError> {
    let state = state.read().unwrap();
    let app_service = state.app_service.read().unwrap();

    // Get config for semantic_search settings
    let config = app_service.get_config().context("Failed to get config")?;
    let config_guard = config.read().unwrap();
    let sem_config = &config_guard.semantic_search;

    // Get indexed count from semantic service if available
    let indexed_count = app_service
        .semantic_service()
        .map(|s| s.indexed_count())
        .unwrap_or(0);

    // Get total bookmarks count
    let total_bookmarks = app_service
        .get_total_count()
        .context("Failed to get total bookmark count")?;

    Ok(axum::Json(SemanticStatusResponse {
        enabled: sem_config.enabled,
        model: sem_config.model.clone(),
        indexed_count,
        total_bookmarks,
    }))
}

// -- Workspace handlers --

async fn list_workspaces(
    State(state): State<Arc<RwLock<SharedState>>>,
) -> Result<axum::Json<Vec<crate::workspaces::Workspace>>, AppError> {
    let state = state.read().unwrap();
    let store = state.workspace_store.read().unwrap();
    Ok(Json(store.list().to_vec()))
}

#[derive(Deserialize)]
struct WorkspaceCreateRequest {
    name: String,
    #[serde(default)]
    filters: Option<crate::workspaces::WorkspaceFilters>,
    #[serde(default)]
    view_prefs: Option<crate::workspaces::ViewPrefs>,
}

async fn create_workspace(
    State(state): State<Arc<RwLock<SharedState>>>,
    Json(payload): Json<WorkspaceCreateRequest>,
) -> Result<(StatusCode, axum::Json<crate::workspaces::Workspace>), AppError> {
    let state = state.read().unwrap();
    let mut store = state.workspace_store.write().unwrap();
    let workspace = store.create(payload.name, payload.filters, payload.view_prefs)?;
    Ok((StatusCode::CREATED, Json(workspace)))
}

#[derive(Deserialize)]
struct WorkspaceUpdateRequest {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    filters: Option<crate::workspaces::WorkspaceFilters>,
    #[serde(default)]
    view_prefs: Option<crate::workspaces::ViewPrefs>,
}

async fn update_workspace(
    State(state): State<Arc<RwLock<SharedState>>>,
    Path(id): Path<String>,
    Json(payload): Json<WorkspaceUpdateRequest>,
) -> Result<axum::Json<crate::workspaces::Workspace>, AppError> {
    let state = state.read().unwrap();
    let mut store = state.workspace_store.write().unwrap();
    let workspace = store.update(&id, payload.name, payload.filters, payload.view_prefs)?;
    Ok(Json(workspace))
}

async fn delete_workspace(
    State(state): State<Arc<RwLock<SharedState>>>,
    Path(id): Path<String>,
) -> Result<StatusCode, AppError> {
    let state = state.read().unwrap();
    let mut store = state.workspace_store.write().unwrap();
    store.delete(&id)?;
    Ok(StatusCode::NO_CONTENT)
}

#[derive(Deserialize)]
struct WorkspaceReorderRequest {
    ids: Vec<String>,
}

async fn reorder_workspaces(
    State(state): State<Arc<RwLock<SharedState>>>,
    Json(payload): Json<WorkspaceReorderRequest>,
) -> Result<StatusCode, AppError> {
    let state = state.read().unwrap();
    let mut store = state.workspace_store.write().unwrap();
    store.reorder(&payload.ids)?;
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::response::IntoResponse;

    #[test]
    fn test_semantic_disabled_error_returns_422() {
        let err = AppError::SemanticDisabled {
            message: "Semantic search is disabled".to_string(),
        };
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    }

    #[test]
    fn test_invalid_threshold_error_returns_400() {
        let err = AppError::InvalidThreshold {
            message: "Threshold must be between 0.0 and 1.0".to_string(),
        };
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn test_model_unavailable_error_returns_503() {
        let err = AppError::ModelUnavailable {
            message: "Model not available".to_string(),
        };
        let response = err.into_response();
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    }

    #[test]
    fn test_threshold_validation_below_zero() {
        // Invalid threshold below 0
        let threshold = -0.1f32;
        assert!(!(0.0..=1.0).contains(&threshold));
    }

    #[test]
    fn test_threshold_validation_above_one() {
        // Invalid threshold above 1
        let threshold = 1.1f32;
        assert!(!(0.0..=1.0).contains(&threshold));
    }

    #[test]
    fn test_threshold_validation_valid_range() {
        // Valid thresholds
        for threshold in [0.0f32, 0.5, 1.0, 0.35] {
            assert!((0.0..=1.0).contains(&threshold));
        }
    }

    #[test]
    fn test_semantic_status_response_serializes_correctly() {
        let response = SemanticStatusResponse {
            enabled: true,
            model: "all-MiniLM-L6-v2".to_string(),
            indexed_count: 42,
            total_bookmarks: 100,
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("\"enabled\":true"));
        assert!(json.contains("\"model\":\"all-MiniLM-L6-v2\""));
        assert!(json.contains("\"indexed_count\":42"));
        assert!(json.contains("\"total_bookmarks\":100"));
    }

    #[test]
    fn test_semantic_status_response_deserializes_correctly() {
        let json = r#"{
            "enabled": false,
            "model": "bge-small-en-v1.5",
            "indexed_count": 0,
            "total_bookmarks": 50
        }"#;

        let response: SemanticStatusResponse = serde_json::from_str(json).unwrap();
        assert!(!response.enabled);
        assert_eq!(response.model, "bge-small-en-v1.5");
        assert_eq!(response.indexed_count, 0);
        assert_eq!(response.total_bookmarks, 50);
    }

    #[test]
    fn test_semantic_status_response_disabled_state() {
        let response = SemanticStatusResponse {
            enabled: false,
            model: "all-MiniLM-L6-v2".to_string(),
            indexed_count: 0,
            total_bookmarks: 25,
        };

        // When disabled, indexed_count is typically 0 but total_bookmarks reflects actual data
        assert!(!response.enabled);
        assert_eq!(response.indexed_count, 0);
        assert!(response.total_bookmarks > 0);
    }

    // =========================================================================
    // HTTP Integration Tests (F.4)
    //
    // These tests verify semantic search via actual HTTP endpoints using
    // tower::ServiceExt::oneshot() to make requests against a test router.
    // =========================================================================

    mod http_integration {
        use super::*;
        use crate::app::backend::{AddOpts, AppBackend, RefreshMetadataOpts};
        use crate::app::errors::AppError as BackendError;
        use crate::app::service::AppService;
        use crate::bookmarks::{Bookmark, BookmarkCreate, BookmarkUpdate, SearchQuery};
        use crate::config::{Config, SemanticSearchConfig};
        use crate::semantic::SemanticSearchService;
        use axum::{body::Body, routing::{get, post}, Router};
        use http_body_util::BodyExt;
        use std::path::PathBuf;
        use std::sync::atomic::{AtomicU64, Ordering};
        use std::sync::{Arc, RwLock};
        use tower::ServiceExt;

        static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

        fn test_dir() -> PathBuf {
            let counter = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
            let path = std::env::temp_dir().join(format!(
                "bb-http-integration-{}-{}",
                std::process::id(),
                counter
            ));
            std::fs::create_dir_all(&path).unwrap();
            path
        }

        /// Mock storage manager for tests
        struct MockStorageManager;

        impl StorageManager for MockStorageManager {
            fn write(&self, _: &str, _: &[u8]) {}
            fn read(&self, _: &str) -> Vec<u8> { vec![] }
            fn exists(&self, _: &str) -> bool { false }
            fn delete(&self, _: &str) -> std::io::Result<()> { Ok(()) }
            fn list(&self) -> Vec<String> { vec![] }
        }

        /// Mock backend that supports search and returns configurable results
        struct MockBackend {
            bookmarks: Vec<Bookmark>,
            config: Config,
        }

        impl MockBackend {
            fn new(bookmarks: Vec<Bookmark>, semantic_enabled: bool) -> Self {
                let mut config = Config::default();
                config.semantic_search.enabled = semantic_enabled;
                Self { bookmarks, config }
            }

            fn with_config(bookmarks: Vec<Bookmark>, semantic_config: SemanticSearchConfig) -> Self {
                let mut config = Config::default();
                config.semantic_search = semantic_config;
                Self { bookmarks, config }
            }
        }

        impl AppBackend for MockBackend {
            fn create(&self, _: BookmarkCreate, _: AddOpts) -> Result<Bookmark, BackendError> {
                unimplemented!()
            }

            fn refresh_metadata(&self, _: u64, _: RefreshMetadataOpts) -> Result<(), BackendError> {
                unimplemented!()
            }

            fn update(&self, _: u64, _: BookmarkUpdate) -> Result<Bookmark, BackendError> {
                unimplemented!()
            }

            fn delete(&self, _: u64) -> Result<(), BackendError> {
                unimplemented!()
            }

            fn search_delete(&self, _: SearchQuery) -> Result<usize, BackendError> {
                unimplemented!()
            }

            fn search_update(&self, _: SearchQuery, _: BookmarkUpdate) -> Result<usize, BackendError> {
                unimplemented!()
            }

            fn total(&self) -> Result<usize, BackendError> {
                Ok(self.bookmarks.len())
            }

            fn tags(&self) -> Result<Vec<String>, BackendError> {
                unimplemented!()
            }

            fn search(&self, _: SearchQuery) -> Result<Vec<Bookmark>, BackendError> {
                Ok(self.bookmarks.clone())
            }

            fn config(&self) -> Result<Arc<RwLock<Config>>, BackendError> {
                Ok(Arc::new(RwLock::new(self.config.clone())))
            }

            fn update_config(&self, _: Config) -> Result<(), BackendError> {
                unimplemented!()
            }
        }

        /// Build a test router with the given app service
        fn test_api_router(app_service: AppService) -> Router {
            let test_dir = test_dir();
            let workspace_store = WorkspaceStore::load(test_dir.to_str().unwrap())
                .expect("failed to load workspace store");
            let shared_state = Arc::new(RwLock::new(SharedState {
                app_service: Arc::new(RwLock::new(app_service)),
                storage_mgr: Arc::new(MockStorageManager),
                workspace_store: Arc::new(RwLock::new(workspace_store)),
            }));

            Router::new()
                .route("/api/bookmarks/search", post(search))
                .route("/api/semantic/status", get(semantic_status))
                .with_state(shared_state)
        }

        fn create_bookmark(id: u64, title: &str, description: &str) -> Bookmark {
            Bookmark {
                id,
                title: title.to_string(),
                url: format!("https://example.com/{}", id),
                description: description.to_string(),
                tags: vec![],
                image_id: None,
                icon_id: None,
            }
        }

        // ---------------------------------------------------------------------
        // Threshold Validation Tests
        // ---------------------------------------------------------------------

        #[tokio::test]
        async fn test_search_invalid_threshold_below_zero_returns_400() {
            let backend = Box::new(MockBackend::new(vec![], true));
            let service = AppService::new(backend);
            let app = test_api_router(service);

            let req = axum::http::Request::builder()
                .method("POST")
                .uri("/api/bookmarks/search")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"semantic": "test", "threshold": -0.5}"#))
                .unwrap();

            let resp = app.oneshot(req).await.unwrap();
            assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

            let body = resp.into_body().collect().await.unwrap().to_bytes();
            let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
            assert_eq!(json["error"], "INVALID_THRESHOLD");
        }

        #[tokio::test]
        async fn test_search_invalid_threshold_above_one_returns_400() {
            let backend = Box::new(MockBackend::new(vec![], true));
            let service = AppService::new(backend);
            let app = test_api_router(service);

            let req = axum::http::Request::builder()
                .method("POST")
                .uri("/api/bookmarks/search")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"semantic": "test", "threshold": 1.5}"#))
                .unwrap();

            let resp = app.oneshot(req).await.unwrap();
            assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

            let body = resp.into_body().collect().await.unwrap().to_bytes();
            let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
            assert_eq!(json["error"], "INVALID_THRESHOLD");
        }

        #[tokio::test]
        async fn test_search_valid_threshold_boundary_zero() {
            let bookmarks = vec![create_bookmark(1, "Test", "Description")];
            let backend = Box::new(MockBackend::new(bookmarks, true));
            let service = AppService::new(backend);
            let app = test_api_router(service);

            let req = axum::http::Request::builder()
                .method("POST")
                .uri("/api/bookmarks/search")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"threshold": 0.0}"#))
                .unwrap();

            let resp = app.oneshot(req).await.unwrap();
            // Should succeed (no semantic query, just threshold - passes through)
            assert_eq!(resp.status(), StatusCode::OK);
        }

        #[tokio::test]
        async fn test_search_valid_threshold_boundary_one() {
            let bookmarks = vec![create_bookmark(1, "Test", "Description")];
            let backend = Box::new(MockBackend::new(bookmarks, true));
            let service = AppService::new(backend);
            let app = test_api_router(service);

            let req = axum::http::Request::builder()
                .method("POST")
                .uri("/api/bookmarks/search")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"threshold": 1.0}"#))
                .unwrap();

            let resp = app.oneshot(req).await.unwrap();
            assert_eq!(resp.status(), StatusCode::OK);
        }

        // ---------------------------------------------------------------------
        // Semantic Disabled State Tests
        // ---------------------------------------------------------------------

        #[tokio::test]
        async fn test_search_semantic_disabled_returns_422() {
            let test_dir = test_dir();

            // Create disabled semantic service
            let config = SemanticSearchConfig {
                enabled: false,
                model: "all-MiniLM-L6-v2".to_string(),
                default_threshold: 0.35,
                embedding_parallelism: "auto".to_string(),
                download_timeout_secs: 300,
                semantic_weight: 0.6,
            };
            let semantic_service = Arc::new(SemanticSearchService::new(config, test_dir.clone()));

            let backend = Box::new(MockBackend::new(vec![], false));
            let service = AppService::with_semantic(backend, semantic_service);
            let app = test_api_router(service);

            // Request with semantic param when disabled
            let req = axum::http::Request::builder()
                .method("POST")
                .uri("/api/bookmarks/search")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"semantic": "machine learning"}"#))
                .unwrap();

            let resp = app.oneshot(req).await.unwrap();
            assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);

            let body = resp.into_body().collect().await.unwrap().to_bytes();
            let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
            assert_eq!(json["error"], "SEMANTIC_DISABLED");

            let _ = std::fs::remove_dir_all(&test_dir);
        }

        // ---------------------------------------------------------------------
        // Semantic Status Endpoint Tests
        // ---------------------------------------------------------------------

        #[tokio::test]
        async fn test_semantic_status_endpoint_enabled() {
            let test_dir = test_dir();

            let config = SemanticSearchConfig {
                enabled: true,
                model: "all-MiniLM-L6-v2".to_string(),
                default_threshold: 0.35,
                embedding_parallelism: "auto".to_string(),
                download_timeout_secs: 300,
                semantic_weight: 0.6,
            };
            let semantic_service = Arc::new(SemanticSearchService::new(config, test_dir.clone()));

            let bookmarks = vec![
                create_bookmark(1, "Test 1", "Desc 1"),
                create_bookmark(2, "Test 2", "Desc 2"),
            ];
            let backend = Box::new(MockBackend::new(bookmarks, true));
            let service = AppService::with_semantic(backend, semantic_service);
            let app = test_api_router(service);

            let req = axum::http::Request::builder()
                .method("GET")
                .uri("/api/semantic/status")
                .body(Body::empty())
                .unwrap();

            let resp = app.oneshot(req).await.unwrap();
            assert_eq!(resp.status(), StatusCode::OK);

            let body = resp.into_body().collect().await.unwrap().to_bytes();
            let json: SemanticStatusResponse = serde_json::from_slice(&body).unwrap();

            assert!(json.enabled);
            assert_eq!(json.model, "all-MiniLM-L6-v2");
            assert_eq!(json.total_bookmarks, 2);
            // indexed_count is 0 because we didn't actually index anything
            assert_eq!(json.indexed_count, 0);

            let _ = std::fs::remove_dir_all(&test_dir);
        }

        #[tokio::test]
        async fn test_semantic_status_endpoint_disabled() {
            let test_dir = test_dir();

            let sem_config = SemanticSearchConfig {
                enabled: false,
                model: "bge-small-en-v1.5".to_string(),
                default_threshold: 0.35,
                embedding_parallelism: "auto".to_string(),
                download_timeout_secs: 300,
                semantic_weight: 0.6,
            };
            let semantic_service = Arc::new(SemanticSearchService::new(sem_config.clone(), test_dir.clone()));

            let bookmarks = vec![create_bookmark(1, "Test", "Desc")];
            // MockBackend config must match the semantic service config for status endpoint
            let backend = Box::new(MockBackend::with_config(bookmarks, sem_config));
            let service = AppService::with_semantic(backend, semantic_service);
            let app = test_api_router(service);

            let req = axum::http::Request::builder()
                .method("GET")
                .uri("/api/semantic/status")
                .body(Body::empty())
                .unwrap();

            let resp = app.oneshot(req).await.unwrap();
            assert_eq!(resp.status(), StatusCode::OK);

            let body = resp.into_body().collect().await.unwrap().to_bytes();
            let json: SemanticStatusResponse = serde_json::from_slice(&body).unwrap();

            assert!(!json.enabled);
            assert_eq!(json.model, "bge-small-en-v1.5");
            assert_eq!(json.total_bookmarks, 1);
            assert_eq!(json.indexed_count, 0);

            let _ = std::fs::remove_dir_all(&test_dir);
        }

        // ---------------------------------------------------------------------
        // Search Passthrough Tests (without semantic service)
        // ---------------------------------------------------------------------

        #[tokio::test]
        async fn test_search_without_semantic_service_passes_through() {
            let bookmarks = vec![
                create_bookmark(1, "Machine Learning", "ML algorithms"),
                create_bookmark(2, "Web Dev", "HTML and CSS"),
            ];
            let backend = Box::new(MockBackend::new(bookmarks, true));
            let service = AppService::new(backend); // No semantic service
            let app = test_api_router(service);

            // Semantic param provided but no service - passes through to backend
            let req = axum::http::Request::builder()
                .method("POST")
                .uri("/api/bookmarks/search")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"semantic": "AI"}"#))
                .unwrap();

            let resp = app.oneshot(req).await.unwrap();
            assert_eq!(resp.status(), StatusCode::OK);

            let body = resp.into_body().collect().await.unwrap().to_bytes();
            let json: Vec<Bookmark> = serde_json::from_slice(&body).unwrap();
            assert_eq!(json.len(), 2);
        }

        #[tokio::test]
        async fn test_search_regular_filters_work() {
            let bookmarks = vec![
                create_bookmark(1, "Rust Guide", "Systems programming"),
                create_bookmark(2, "Python Tutorial", "Data science"),
            ];
            let backend = Box::new(MockBackend::new(bookmarks, true));
            let service = AppService::new(backend);
            let app = test_api_router(service);

            // Regular search without semantic
            let req = axum::http::Request::builder()
                .method("POST")
                .uri("/api/bookmarks/search")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"title": "Rust"}"#))
                .unwrap();

            let resp = app.oneshot(req).await.unwrap();
            assert_eq!(resp.status(), StatusCode::OK);

            let body = resp.into_body().collect().await.unwrap().to_bytes();
            let json: Vec<Bookmark> = serde_json::from_slice(&body).unwrap();
            // Mock backend returns all bookmarks (doesn't filter)
            assert_eq!(json.len(), 2);
        }

        // -----------------------------------------------------------------
        // Workspace HTTP Integration Tests
        // -----------------------------------------------------------------

        fn workspace_api_router() -> Router {
            let dir = test_dir();
            let workspace_store = WorkspaceStore::load(dir.to_str().unwrap())
                .expect("failed to load workspace store");
            let backend = Box::new(MockBackend::new(vec![], false));
            let service = AppService::new(backend);
            let shared_state = Arc::new(RwLock::new(SharedState {
                app_service: Arc::new(RwLock::new(service)),
                storage_mgr: Arc::new(MockStorageManager),
                workspace_store: Arc::new(RwLock::new(workspace_store)),
            }));

            Router::new()
                .route("/api/workspaces", get(list_workspaces))
                .route("/api/workspaces", post(create_workspace))
                .route("/api/workspaces/:id", put(update_workspace))
                .route("/api/workspaces/:id", delete_method(delete_workspace))
                .with_state(shared_state)
        }

        #[tokio::test]
        async fn test_workspace_list_empty() {
            let app = workspace_api_router();
            let req = axum::http::Request::builder()
                .method("GET")
                .uri("/api/workspaces")
                .body(Body::empty())
                .unwrap();
            let resp = app.oneshot(req).await.unwrap();
            assert_eq!(resp.status(), StatusCode::OK);

            let body = resp.into_body().collect().await.unwrap().to_bytes();
            let json: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();
            assert!(json.is_empty());
        }

        #[tokio::test]
        async fn test_workspace_create_success() {
            let app = workspace_api_router();
            let req = axum::http::Request::builder()
                .method("POST")
                .uri("/api/workspaces")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"name":"Dev"}"#))
                .unwrap();
            let resp = app.oneshot(req).await.unwrap();
            assert_eq!(resp.status(), StatusCode::CREATED);

            let body = resp.into_body().collect().await.unwrap().to_bytes();
            let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
            assert_eq!(json["name"], "Dev");
            assert!(json["id"].as_str().map_or(false, |s| !s.is_empty()));
        }

        #[tokio::test]
        async fn test_workspace_create_validation_error() {
            let app = workspace_api_router();
            let req = axum::http::Request::builder()
                .method("POST")
                .uri("/api/workspaces")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"name":""}"#))
                .unwrap();
            let resp = app.oneshot(req).await.unwrap();
            assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        }

        #[tokio::test]
        async fn test_workspace_create_invalid_regex() {
            let app = workspace_api_router();
            let req = axum::http::Request::builder()
                .method("POST")
                .uri("/api/workspaces")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"name":"Bad","filters":{"url_pattern":"[bad"}}"#,
                ))
                .unwrap();
            let resp = app.oneshot(req).await.unwrap();
            assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
        }

        #[tokio::test]
        async fn test_workspace_update_not_found() {
            let app = workspace_api_router();
            let req = axum::http::Request::builder()
                .method("PUT")
                .uri("/api/workspaces/nonexistent")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"name":"X"}"#))
                .unwrap();
            let resp = app.oneshot(req).await.unwrap();
            assert_eq!(resp.status(), StatusCode::NOT_FOUND);
        }

        #[tokio::test]
        async fn test_workspace_delete_not_found() {
            let app = workspace_api_router();
            let req = axum::http::Request::builder()
                .method("DELETE")
                .uri("/api/workspaces/nonexistent")
                .body(Body::empty())
                .unwrap();
            let resp = app.oneshot(req).await.unwrap();
            assert_eq!(resp.status(), StatusCode::NOT_FOUND);
        }

        #[tokio::test]
        async fn test_workspace_full_crud_flow() {
            let dir = test_dir();
            let workspace_store = WorkspaceStore::load(dir.to_str().unwrap()).unwrap();
            let backend = Box::new(MockBackend::new(vec![], false));
            let service = AppService::new(backend);
            let shared_state = Arc::new(RwLock::new(SharedState {
                app_service: Arc::new(RwLock::new(service)),
                storage_mgr: Arc::new(MockStorageManager),
                workspace_store: Arc::new(RwLock::new(workspace_store)),
            }));

            let app = Router::new()
                .route("/api/workspaces", get(list_workspaces))
                .route("/api/workspaces", post(create_workspace))
                .route("/api/workspaces/:id", put(update_workspace))
                .route("/api/workspaces/:id", delete_method(delete_workspace))
                .with_state(shared_state.clone());

            // Create
            let req = axum::http::Request::builder()
                .method("POST")
                .uri("/api/workspaces")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"name":"Flow"}"#))
                .unwrap();
            let resp = app.clone().oneshot(req).await.unwrap();
            assert_eq!(resp.status(), StatusCode::CREATED);
            let body = resp.into_body().collect().await.unwrap().to_bytes();
            let created: serde_json::Value = serde_json::from_slice(&body).unwrap();
            let id = created["id"].as_str().unwrap();

            // List — should have 1
            let req = axum::http::Request::builder()
                .method("GET")
                .uri("/api/workspaces")
                .body(Body::empty())
                .unwrap();
            let resp = app.clone().oneshot(req).await.unwrap();
            let body = resp.into_body().collect().await.unwrap().to_bytes();
            let list: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();
            assert_eq!(list.len(), 1);

            // Update
            let req = axum::http::Request::builder()
                .method("PUT")
                .uri(&format!("/api/workspaces/{id}"))
                .header("content-type", "application/json")
                .body(Body::from(r#"{"name":"Updated"}"#))
                .unwrap();
            let resp = app.clone().oneshot(req).await.unwrap();
            assert_eq!(resp.status(), StatusCode::OK);
            let body = resp.into_body().collect().await.unwrap().to_bytes();
            let updated: serde_json::Value = serde_json::from_slice(&body).unwrap();
            assert_eq!(updated["name"], "Updated");

            // Delete
            let req = axum::http::Request::builder()
                .method("DELETE")
                .uri(&format!("/api/workspaces/{id}"))
                .body(Body::empty())
                .unwrap();
            let resp = app.clone().oneshot(req).await.unwrap();
            assert_eq!(resp.status(), StatusCode::NO_CONTENT);

            // List — should be empty
            let req = axum::http::Request::builder()
                .method("GET")
                .uri("/api/workspaces")
                .body(Body::empty())
                .unwrap();
            let resp = app.oneshot(req).await.unwrap();
            let body = resp.into_body().collect().await.unwrap().to_bytes();
            let list: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();
            assert!(list.is_empty());
        }
    }
}
