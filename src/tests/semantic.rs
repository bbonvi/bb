//! Integration tests for the semantic search module.
//!
//! These tests require model download and are marked #[ignore] by default.
//! Run with: cargo test --test semantic -- --ignored

use crate::semantic::{
    content_hash, preprocess_content, EmbeddingModel, VectorIndex, VectorStorage,
};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

fn test_dir() -> PathBuf {
    let counter = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
    let path = std::env::temp_dir().join(format!(
        "bb-semantic-integration-{}-{}",
        std::process::id(),
        counter
    ));
    std::fs::create_dir_all(&path).unwrap();
    path
}

/// Test the full embedding → storage → search flow.
#[test]
#[ignore = "requires model download (~23MB)"]
fn test_embedding_storage_search_flow() {
    let test_dir = test_dir();
    let vectors_path = test_dir.join("vectors.bin");

    // 1. Initialize embedding model
    let model = EmbeddingModel::new("all-MiniLM-L6-v2", test_dir.clone(), None)
        .expect("Failed to initialize embedding model");

    assert_eq!(model.dimensions(), 384);

    // 2. Create embeddings for test bookmarks
    let bookmarks = vec![
        ("Machine Learning Tutorial", "An introduction to ML algorithms and neural networks"),
        ("Rust Programming Guide", "Learn the Rust programming language with examples"),
        ("Deep Learning with Python", "Build neural networks using TensorFlow and Keras"),
        ("Web Development Basics", "HTML, CSS, and JavaScript fundamentals"),
    ];

    let mut index = VectorIndex::new(model.dimensions());

    for (id, (title, description)) in bookmarks.iter().enumerate() {
        let content = preprocess_content(title, description).unwrap();
        let embedding = model.embed(&content).expect("Failed to generate embedding");

        let hash = content_hash(title, description);
        index.insert(id as u64, hash, embedding).expect("Failed to insert");
    }

    assert_eq!(index.len(), 4);

    // 3. Save to storage
    let storage = VectorStorage::new(vectors_path.clone());
    let model_id = model.model_id_hash();
    storage.save(&index, &model_id).expect("Failed to save");

    assert!(storage.exists());

    // 4. Reload from storage
    let loaded_index = storage.load(&model_id, model.dimensions())
        .expect("Failed to load");
    assert_eq!(loaded_index.len(), 4);

    // 5. Test semantic search
    let query = "artificial intelligence machine learning";
    let query_embedding = model.embed(query).expect("Failed to embed query");

    let results = loaded_index
        .search(&query_embedding, None, 0.3, 10)
        .expect("Search failed");

    // Should find ML-related bookmarks with higher scores
    assert!(!results.is_empty());

    // First result should be machine learning related
    let first_result = &results[0];
    assert!(
        first_result.id == 0 || first_result.id == 2,
        "Expected ML-related bookmark, got id={}",
        first_result.id
    );

    // 6. Test search with candidate filtering
    let candidates = vec![1, 3]; // Only Rust and Web dev
    let filtered_results = loaded_index
        .search(&query_embedding, Some(&candidates), 0.0, 10)
        .expect("Search failed");

    // Results should only contain IDs from candidates
    for result in &filtered_results {
        assert!(candidates.contains(&result.id));
    }

    // Cleanup
    let _ = std::fs::remove_dir_all(&test_dir);
}

/// Test that similar content produces similar embeddings.
#[test]
#[ignore = "requires model download (~23MB)"]
fn test_semantic_similarity() {
    let test_dir = test_dir();

    let model = EmbeddingModel::new("all-MiniLM-L6-v2", test_dir.clone(), None)
        .expect("Failed to initialize embedding model");

    // Similar texts
    let text1 = "Introduction to machine learning and AI";
    let text2 = "Getting started with artificial intelligence and ML";

    // Different text
    let text3 = "Best recipes for chocolate cake baking";

    let emb1 = model.embed(text1).unwrap();
    let emb2 = model.embed(text2).unwrap();
    let emb3 = model.embed(text3).unwrap();

    let sim_1_2 = cosine_similarity(&emb1, &emb2);
    let sim_1_3 = cosine_similarity(&emb1, &emb3);

    // Similar texts should have higher similarity
    assert!(
        sim_1_2 > sim_1_3,
        "Similar texts should have higher similarity: {} vs {}",
        sim_1_2,
        sim_1_3
    );

    // Similar texts should be above threshold
    assert!(sim_1_2 > 0.5, "Similar texts should be above 0.5: {}", sim_1_2);

    // Different texts should be below threshold
    assert!(sim_1_3 < 0.5, "Different texts should be below 0.5: {}", sim_1_3);

    let _ = std::fs::remove_dir_all(&test_dir);
}

/// Test preprocessing edge cases.
#[test]
fn test_preprocessing_edge_cases() {
    // Unicode handling
    let content = preprocess_content("日本語タイトル", "Unicode description 中文");
    assert!(content.is_some());

    // Very long content
    let long_title = "A".repeat(1000);
    let content = preprocess_content(&long_title, "Short");
    let content = content.unwrap();
    assert!(content.len() <= 512);
    assert!(content.ends_with("..."));

    // Whitespace only
    assert!(preprocess_content("   ", "\t\n").is_none());
}

fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();
    dot / (norm_a * norm_b)
}
