//! Semantic search service for bookmark similarity search.
//!
//! Provides a high-level interface for semantic search operations:
//! - Lazy-loads the embedding model and vector index
//! - Coordinates embedding generation and similarity search
//! - Thread-safe with interior mutability for lazy initialization

use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;

use crate::config::SemanticSearchConfig;
use crate::semantic::embeddings::EmbeddingError;
use crate::semantic::index::{IndexError, SearchResult, VectorIndex};
use crate::semantic::storage::{VectorStorage, VectorStorageError};
use crate::semantic::EmbeddingModel;

/// Errors that can occur during semantic search operations.
#[derive(Debug, thiserror::Error)]
pub enum SemanticSearchError {
    #[error("Semantic search is disabled")]
    Disabled,

    #[error("Embedding error: {0}")]
    Embedding(#[from] EmbeddingError),

    #[error("Index error: {0}")]
    Index(#[from] IndexError),

    #[error("Storage error: {0}")]
    Storage(#[from] VectorStorageError),

    #[error("Service not initialized")]
    NotInitialized,

    #[error("Internal error: {0}")]
    Internal(String),
}

/// Lazy-loaded semantic search components.
struct SemanticState {
    model: EmbeddingModel,
    index: VectorIndex,
    storage: VectorStorage,
}

/// Service for performing semantic search on bookmarks.
///
/// Lazily loads the embedding model and vector index on first use.
/// Thread-safe through interior mutability.
pub struct SemanticSearchService {
    config: SemanticSearchConfig,
    base_path: PathBuf,
    /// Lazily-initialized state. Uses Mutex<Option<_>> instead of OnceLock
    /// because get_or_try_init is unstable.
    state: Mutex<Option<SemanticState>>,
}

impl SemanticSearchService {
    /// Create a new semantic search service.
    ///
    /// The service is created in an uninitialized state and will lazy-load
    /// the embedding model and vector index on first search.
    ///
    /// # Arguments
    /// * `config` - Semantic search configuration
    /// * `base_path` - Base directory for data files (vectors.bin, models/)
    pub fn new(config: SemanticSearchConfig, base_path: PathBuf) -> Self {
        Self {
            config,
            base_path,
            state: Mutex::new(None),
        }
    }

    /// Check if semantic search is enabled.
    pub fn is_enabled(&self) -> bool {
        self.config.enabled
    }

    /// Get the configured default threshold.
    pub fn default_threshold(&self) -> f32 {
        self.config.default_threshold
    }

    /// Search for semantically similar bookmarks.
    ///
    /// # Arguments
    /// * `query` - The search query text
    /// * `candidate_ids` - Optional set of bookmark IDs to search within
    /// * `threshold` - Minimum similarity score (uses config default if None)
    /// * `limit` - Maximum results to return
    ///
    /// # Returns
    /// Bookmark IDs sorted by similarity (highest first).
    pub fn search(
        &self,
        query: &str,
        candidate_ids: Option<&[u64]>,
        threshold: Option<f32>,
        limit: usize,
    ) -> Result<Vec<u64>, SemanticSearchError> {
        if !self.config.enabled {
            return Err(SemanticSearchError::Disabled);
        }

        self.ensure_initialized()?;

        let mut guard = self
            .state
            .lock()
            .map_err(|e| SemanticSearchError::Internal(format!("Lock poisoned: {}", e)))?;

        let state = guard
            .as_mut()
            .ok_or(SemanticSearchError::NotInitialized)?;

        // Generate query embedding
        let query_embedding = state.model.embed(query)?;

        // Search the index
        let threshold = threshold.unwrap_or(self.config.default_threshold);
        let results = state
            .index
            .search(&query_embedding, candidate_ids, threshold, limit)?;

        Ok(results.into_iter().map(|r| r.id).collect())
    }

    /// Search and return full results with scores.
    ///
    /// Same as `search()` but returns (id, score) pairs.
    pub fn search_with_scores(
        &self,
        query: &str,
        candidate_ids: Option<&[u64]>,
        threshold: Option<f32>,
        limit: usize,
    ) -> Result<Vec<SearchResult>, SemanticSearchError> {
        if !self.config.enabled {
            return Err(SemanticSearchError::Disabled);
        }

        self.ensure_initialized()?;

        let mut guard = self
            .state
            .lock()
            .map_err(|e| SemanticSearchError::Internal(format!("Lock poisoned: {}", e)))?;

        let state = guard
            .as_mut()
            .ok_or(SemanticSearchError::NotInitialized)?;

        let query_embedding = state.model.embed(query)?;
        let threshold = threshold.unwrap_or(self.config.default_threshold);
        let results = state
            .index
            .search(&query_embedding, candidate_ids, threshold, limit)?;

        Ok(results)
    }

    /// Get the number of indexed entries.
    ///
    /// Returns 0 if not yet initialized.
    pub fn indexed_count(&self) -> usize {
        self.state
            .lock()
            .ok()
            .and_then(|guard| guard.as_ref().map(|s| s.index.len()))
            .unwrap_or(0)
    }

    /// Check if the service has been initialized.
    pub fn is_initialized(&self) -> bool {
        self.state
            .lock()
            .ok()
            .map(|guard| guard.is_some())
            .unwrap_or(false)
    }

    /// Force initialization of the service.
    ///
    /// Normally initialization happens lazily on first search.
    /// Call this to eagerly load the model and index.
    pub fn initialize(&self) -> Result<(), SemanticSearchError> {
        if !self.config.enabled {
            return Err(SemanticSearchError::Disabled);
        }
        self.ensure_initialized()
    }

    /// Get a mutable reference to the index (for maintenance operations).
    ///
    /// # Warning
    /// This is intended for index maintenance (add/remove entries).
    /// The caller must ensure consistency with storage.
    pub fn with_index_mut<F, R>(&self, f: F) -> Result<R, SemanticSearchError>
    where
        F: FnOnce(&mut VectorIndex, &EmbeddingModel) -> R,
    {
        if !self.config.enabled {
            return Err(SemanticSearchError::Disabled);
        }

        self.ensure_initialized()?;

        let mut guard = self
            .state
            .lock()
            .map_err(|e| SemanticSearchError::Internal(format!("Lock poisoned: {}", e)))?;

        let state = guard
            .as_mut()
            .ok_or(SemanticSearchError::NotInitialized)?;

        // Extract references to avoid simultaneous borrow issues
        let index = &mut state.index;
        let model = &state.model;
        Ok(f(index, model))
    }

    /// Save the current index state to storage.
    pub fn save_index(&self) -> Result<(), SemanticSearchError> {
        if !self.config.enabled {
            return Err(SemanticSearchError::Disabled);
        }

        self.ensure_initialized()?;

        let guard = self
            .state
            .lock()
            .map_err(|e| SemanticSearchError::Internal(format!("Lock poisoned: {}", e)))?;

        let state = guard.as_ref().ok_or(SemanticSearchError::NotInitialized)?;

        let model_id = state.model.model_id_hash();
        state.storage.save(&state.index, &model_id)?;

        Ok(())
    }

    /// Ensure the service is initialized, initializing if needed.
    fn ensure_initialized(&self) -> Result<(), SemanticSearchError> {
        let mut guard = self
            .state
            .lock()
            .map_err(|e| SemanticSearchError::Internal(format!("Lock poisoned: {}", e)))?;

        if guard.is_none() {
            *guard = Some(self.do_init()?);
        }

        Ok(())
    }

    /// Perform actual initialization.
    fn do_init(&self) -> Result<SemanticState, SemanticSearchError> {
        log::info!(
            "Initializing semantic search with model '{}'",
            self.config.model
        );

        // Create the embedding model
        let timeout = Duration::from_secs(self.config.download_timeout_secs);
        let model = EmbeddingModel::new(&self.config.model, self.base_path.clone(), Some(timeout))?;

        let model_id = model.model_id_hash();
        let dimensions = model.dimensions();

        // Set up storage
        let vectors_path = self.base_path.join("vectors.bin");
        let storage = VectorStorage::new(vectors_path);

        // Load or create index
        let index = if storage.exists() {
            match storage.load(&model_id, dimensions) {
                Ok(idx) => {
                    log::info!("Loaded {} vectors from storage", idx.len());
                    idx
                }
                Err(VectorStorageError::ModelMismatch) => {
                    log::warn!("Model changed, creating fresh index");
                    VectorIndex::new(dimensions)
                }
                Err(VectorStorageError::VersionMismatch(file_ver, _)) => {
                    log::warn!(
                        "Storage version {} unsupported, creating fresh index",
                        file_ver
                    );
                    VectorIndex::new(dimensions)
                }
                Err(e) => {
                    log::error!("Failed to load vectors: {}", e);
                    return Err(e.into());
                }
            }
        } else {
            log::info!("No existing index, starting fresh");
            VectorIndex::new(dimensions)
        };

        Ok(SemanticState {
            model,
            index,
            storage,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config(enabled: bool) -> SemanticSearchConfig {
        SemanticSearchConfig {
            enabled,
            model: "all-MiniLM-L6-v2".to_string(),
            default_threshold: 0.35,
            embedding_parallelism: "auto".to_string(),
            download_timeout_secs: 300,
        }
    }

    #[test]
    fn test_disabled_service_returns_error() {
        let config = test_config(false);
        let service = SemanticSearchService::new(config, PathBuf::from("/tmp"));

        let result = service.search("test query", None, None, 10);
        assert!(matches!(result, Err(SemanticSearchError::Disabled)));
    }

    #[test]
    fn test_is_enabled() {
        let enabled_config = test_config(true);
        let disabled_config = test_config(false);

        let service1 = SemanticSearchService::new(enabled_config, PathBuf::from("/tmp"));
        let service2 = SemanticSearchService::new(disabled_config, PathBuf::from("/tmp"));

        assert!(service1.is_enabled());
        assert!(!service2.is_enabled());
    }

    #[test]
    fn test_default_threshold() {
        let mut config = test_config(true);
        config.default_threshold = 0.5;

        let service = SemanticSearchService::new(config, PathBuf::from("/tmp"));
        assert!((service.default_threshold() - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn test_not_initialized_initially() {
        let config = test_config(true);
        let service = SemanticSearchService::new(config, PathBuf::from("/tmp"));

        assert!(!service.is_initialized());
        assert_eq!(service.indexed_count(), 0);
    }

    #[test]
    fn test_initialize_disabled_returns_error() {
        let config = test_config(false);
        let service = SemanticSearchService::new(config, PathBuf::from("/tmp"));

        let result = service.initialize();
        assert!(matches!(result, Err(SemanticSearchError::Disabled)));
    }

    // Integration tests require model download
    #[test]
    #[ignore = "requires model download"]
    fn test_search_integration() {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);

        let test_dir = std::env::temp_dir().join(format!(
            "bb-semantic-service-test-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::SeqCst)
        ));
        std::fs::create_dir_all(&test_dir).unwrap();

        let config = test_config(true);
        let service = SemanticSearchService::new(config, test_dir.clone());

        // Initialize and add some test vectors
        service.initialize().unwrap();
        assert!(service.is_initialized());

        // Add test entries
        service
            .with_index_mut(|index, model| {
                let emb1 = model.embed("machine learning artificial intelligence").unwrap();
                let emb2 = model.embed("cooking recipes food").unwrap();
                let emb3 = model.embed("deep neural networks").unwrap();

                index.insert(1, 100, emb1).unwrap();
                index.insert(2, 200, emb2).unwrap();
                index.insert(3, 300, emb3).unwrap();
            })
            .unwrap();

        // Search for ML-related content
        let results = service.search("AI and deep learning", None, Some(0.3), 10).unwrap();

        // Should find ML-related bookmarks, not cooking
        assert!(!results.is_empty());
        assert!(results.contains(&1) || results.contains(&3));

        // Cleanup
        let _ = std::fs::remove_dir_all(&test_dir);
    }

    #[test]
    #[ignore = "requires model download"]
    fn test_save_and_reload() {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);

        let test_dir = std::env::temp_dir().join(format!(
            "bb-semantic-save-test-{}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::SeqCst)
        ));
        std::fs::create_dir_all(&test_dir).unwrap();

        let config = test_config(true);

        // Create service and add data
        {
            let service = SemanticSearchService::new(config.clone(), test_dir.clone());
            service.initialize().unwrap();

            service
                .with_index_mut(|index, model| {
                    let emb = model.embed("test content").unwrap();
                    index.insert(42, 12345, emb).unwrap();
                })
                .unwrap();

            service.save_index().unwrap();
        }

        // Create new service and verify data persists
        {
            let service = SemanticSearchService::new(config, test_dir.clone());
            service.initialize().unwrap();

            assert_eq!(service.indexed_count(), 1);
        }

        // Cleanup
        let _ = std::fs::remove_dir_all(&test_dir);
    }
}
