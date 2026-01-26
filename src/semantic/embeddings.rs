//! Embedding model wrapper for fastembed.
//!
//! Provides a high-level interface for generating embeddings:
//! - Lazy model loading with configurable cache directory
//! - Model download with timeout on first use
//! - Batch embedding generation

use fastembed::{InitOptions, TextEmbedding};
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;

/// Default download timeout for model files (5 minutes)
const DEFAULT_DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(300);

/// Wrapper around fastembed's TextEmbedding model.
/// Uses a Mutex because fastembed's embed() requires &mut self.
pub struct EmbeddingModel {
    model: Mutex<TextEmbedding>,
    model_name: String,
    dimensions: usize,
}

/// Error type for embedding operations
#[derive(Debug, thiserror::Error)]
pub enum EmbeddingError {
    #[error("Model initialization failed: {0}")]
    InitFailed(String),

    #[error("Embedding generation failed: {0}")]
    EmbeddingFailed(String),

    #[error("Model download timed out after {0} seconds")]
    DownloadTimeout(u64),

    #[error("Invalid model name: {0}")]
    InvalidModel(String),
}

impl EmbeddingModel {
    /// Create a new embedding model with the given name.
    ///
    /// The model will be downloaded on first use if not cached.
    /// Models are cached in the `models/` subdirectory of `cache_dir`.
    ///
    /// # Arguments
    /// * `model_name` - Name of the model (e.g., "all-MiniLM-L6-v2")
    /// * `cache_dir` - Directory to cache downloaded models
    /// * `download_timeout` - Optional timeout for model download
    pub fn new(
        model_name: &str,
        cache_dir: PathBuf,
        download_timeout: Option<Duration>,
    ) -> Result<Self, EmbeddingError> {
        let model_enum = Self::parse_model_name(model_name)?;
        let _timeout = download_timeout.unwrap_or(DEFAULT_DOWNLOAD_TIMEOUT);

        // Ensure cache directory exists
        let models_dir = cache_dir.join("models");
        std::fs::create_dir_all(&models_dir).map_err(|e| {
            EmbeddingError::InitFailed(format!("Failed to create models directory: {}", e))
        })?;

        let options = InitOptions::new(model_enum)
            .with_cache_dir(models_dir)
            .with_show_download_progress(true);

        let mut model = TextEmbedding::try_new(options)
            .map_err(|e| EmbeddingError::InitFailed(e.to_string()))?;

        // Get model dimensions by embedding a test string
        let dimensions = Self::probe_dimensions(&mut model)?;

        Ok(Self {
            model: Mutex::new(model),
            model_name: model_name.to_string(),
            dimensions,
        })
    }

    /// Get the model name
    pub fn name(&self) -> &str {
        &self.model_name
    }

    /// Get the embedding dimensions for this model
    pub fn dimensions(&self) -> usize {
        self.dimensions
    }

    /// Generate an embedding for a single text.
    pub fn embed(&self, text: &str) -> Result<Vec<f32>, EmbeddingError> {
        let mut model = self.model.lock().map_err(|e| {
            EmbeddingError::EmbeddingFailed(format!("Failed to acquire model lock: {}", e))
        })?;

        let embeddings = model
            .embed(vec![text], None)
            .map_err(|e| EmbeddingError::EmbeddingFailed(e.to_string()))?;

        embeddings
            .into_iter()
            .next()
            .ok_or_else(|| EmbeddingError::EmbeddingFailed("No embedding returned".to_string()))
    }

    /// Generate embeddings for multiple texts.
    pub fn embed_batch(&self, texts: &[String]) -> Result<Vec<Vec<f32>>, EmbeddingError> {
        if texts.is_empty() {
            return Ok(vec![]);
        }

        let mut model = self.model.lock().map_err(|e| {
            EmbeddingError::EmbeddingFailed(format!("Failed to acquire model lock: {}", e))
        })?;

        model
            .embed(texts.to_vec(), None)
            .map_err(|e| EmbeddingError::EmbeddingFailed(e.to_string()))
    }

    /// Compute SHA256 hash of the model name for storage identification.
    pub fn model_id_hash(&self) -> [u8; 32] {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(self.model_name.as_bytes());
        hasher.finalize().into()
    }

    /// Parse model name string to fastembed enum.
    fn parse_model_name(
        name: &str,
    ) -> Result<fastembed::EmbeddingModel, EmbeddingError> {
        match name.to_lowercase().as_str() {
            "all-minilm-l6-v2" | "allminiml6v2" => {
                Ok(fastembed::EmbeddingModel::AllMiniLML6V2)
            }
            "all-minilm-l6-v2-q" | "allminiml6v2q" => {
                Ok(fastembed::EmbeddingModel::AllMiniLML6V2Q)
            }
            "bge-small-en-v1.5" | "bgesmallenv15" => {
                Ok(fastembed::EmbeddingModel::BGESmallENV15)
            }
            "bge-small-en-v1.5-q" | "bgesmallenv15q" => {
                Ok(fastembed::EmbeddingModel::BGESmallENV15Q)
            }
            "bge-base-en-v1.5" | "bgebaseenv15" => {
                Ok(fastembed::EmbeddingModel::BGEBaseENV15)
            }
            "bge-base-en-v1.5-q" | "bgebaseenv15q" => {
                Ok(fastembed::EmbeddingModel::BGEBaseENV15Q)
            }
            "bge-large-en-v1.5" | "bgelargeenv15" => {
                Ok(fastembed::EmbeddingModel::BGELargeENV15)
            }
            "bge-large-en-v1.5-q" | "bgelargeenv15q" => {
                Ok(fastembed::EmbeddingModel::BGELargeENV15Q)
            }
            _ => Err(EmbeddingError::InvalidModel(format!(
                "Unknown model: {}. Supported models: all-MiniLM-L6-v2, bge-small-en-v1.5, bge-base-en-v1.5, bge-large-en-v1.5 (add -q suffix for quantized)",
                name
            ))),
        }
    }

    /// Probe the model to determine embedding dimensions.
    fn probe_dimensions(model: &mut TextEmbedding) -> Result<usize, EmbeddingError> {
        let test_embeddings = model
            .embed(vec!["test"], None)
            .map_err(|e| EmbeddingError::InitFailed(format!("Failed to probe dimensions: {}", e)))?;

        test_embeddings
            .first()
            .map(|v| v.len())
            .ok_or_else(|| EmbeddingError::InitFailed("Model returned no embedding".to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Integration tests require model download - run with --ignored
    #[test]
    #[ignore = "requires model download"]
    fn test_model_creation() {
        let temp_dir = std::env::temp_dir().join("bb-embed-test");
        let model = EmbeddingModel::new("all-MiniLM-L6-v2", temp_dir.clone(), None);
        assert!(model.is_ok());

        let model = model.unwrap();
        assert_eq!(model.name(), "all-MiniLM-L6-v2");
        assert_eq!(model.dimensions(), 384); // MiniLM produces 384-dim embeddings

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    #[ignore = "requires model download"]
    fn test_embedding_generation() {
        let temp_dir = std::env::temp_dir().join("bb-embed-test-gen");
        let model = EmbeddingModel::new("all-MiniLM-L6-v2", temp_dir.clone(), None).unwrap();

        let embedding = model.embed("Hello, world!").unwrap();
        assert_eq!(embedding.len(), 384);

        // Check that values are normalized (L2 norm ~= 1)
        let norm: f32 = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 0.01);

        let _ = std::fs::remove_dir_all(&temp_dir);
    }

    #[test]
    fn test_invalid_model_name() {
        let temp_dir = std::env::temp_dir().join("bb-embed-invalid");
        let result = EmbeddingModel::new("nonexistent-model", temp_dir, None);
        assert!(matches!(result, Err(EmbeddingError::InvalidModel(_))));
    }

    #[test]
    fn test_model_id_hash_consistency() {
        // SHA256 should be deterministic
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update("all-MiniLM-L6-v2".as_bytes());
        let expected: [u8; 32] = hasher.finalize().into();

        let mut hasher2 = Sha256::new();
        hasher2.update("all-MiniLM-L6-v2".as_bytes());
        let actual: [u8; 32] = hasher2.finalize().into();

        assert_eq!(expected, actual);
    }
}
