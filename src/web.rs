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
    metadata::MetaOptions,
    storage::{self, StorageManager},
};
use anyhow::Context;
use axum::{
    extract::{DefaultBodyLimit, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
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
}

async fn start_app(app_service: AppService, base_path: &str) {
    let storage_mgr = Arc::new(storage::BackendLocal::new(&format!("{base_path}/uploads")));
    let shared_state = Arc::new(RwLock::new(SharedState {
        app_service: Arc::new(RwLock::new(app_service)),
        storage_mgr,
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

    #[error("{message}")]
    SemanticDisabled { message: String },

    #[error("{message}")]
    InvalidThreshold { message: String },

    #[error("{message}")]
    ModelUnavailable { message: String },

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
}
