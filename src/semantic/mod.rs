//! Semantic search infrastructure for bookmark embeddings.
//!
//! This module provides local semantic search capabilities using fastembed-rs
//! for generating embeddings and in-memory vector similarity search.
//!
//! # Architecture
//!
//! - `embeddings`: Wraps fastembed for embedding generation
//! - `index`: In-memory vector index with cosine similarity search
//! - `storage`: Binary file I/O for vectors.bin persistence
//! - `preprocess`: Text preprocessing for embedding input
//! - `service`: High-level semantic search service

pub mod embeddings;
mod index;
mod preprocess;
mod service;
mod storage;

pub use embeddings::EmbeddingModel;
pub use index::{SearchResult, VectorIndex};
pub use preprocess::{content_hash, preprocess_content};
pub use service::{ReconcileResult, SemanticSearchError, SemanticSearchService};
pub use storage::{VectorStorage, VectorStorageError};

/// Default embedding model name (bge-base offers +13% accuracy vs MiniLM)
pub const DEFAULT_MODEL: &str = "bge-base-en-v1.5";

/// Default similarity threshold for semantic search
pub const DEFAULT_THRESHOLD: f32 = 0.35;
