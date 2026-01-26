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

// =============================================================================
// Backend Search Integration Tests (C.4)
//
// These tests verify the filter-then-rank behavior of AppService.search_bookmarks
// when semantic search is enabled.
// =============================================================================

mod backend_integration {
    use crate::app::backend::{AddOpts, AppBackend, RefreshMetadataOpts};
    use crate::app::errors::AppError;
    use crate::app::service::AppService;
    use crate::bookmarks::{Bookmark, BookmarkCreate, BookmarkUpdate, SearchQuery};
    use crate::config::{Config, SemanticSearchConfig};
    use crate::semantic::{content_hash, preprocess_content, SemanticSearchService};
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::{Arc, RwLock};

    static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn test_dir() -> PathBuf {
        let counter = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
        let path = std::env::temp_dir().join(format!(
            "bb-backend-integration-{}-{}",
            std::process::id(),
            counter
        ));
        std::fs::create_dir_all(&path).unwrap();
        path
    }

    fn enabled_semantic_config() -> SemanticSearchConfig {
        SemanticSearchConfig {
            enabled: true,
            model: "all-MiniLM-L6-v2".to_string(),
            default_threshold: 0.35,
            embedding_parallelism: "auto".to_string(),
            download_timeout_secs: 300,
        }
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

    /// Mock backend that filters by title substring when query.title is set.
    struct FilterableMockBackend {
        bookmarks: Vec<Bookmark>,
    }

    impl FilterableMockBackend {
        fn new(bookmarks: Vec<Bookmark>) -> Self {
            Self { bookmarks }
        }
    }

    impl AppBackend for FilterableMockBackend {
        fn create(&self, _: BookmarkCreate, _: AddOpts) -> Result<Bookmark, AppError> {
            unimplemented!()
        }

        fn refresh_metadata(&self, _: u64, _: RefreshMetadataOpts) -> Result<(), AppError> {
            unimplemented!()
        }

        fn update(&self, _: u64, _: BookmarkUpdate) -> Result<Bookmark, AppError> {
            unimplemented!()
        }

        fn delete(&self, _: u64) -> Result<(), AppError> {
            unimplemented!()
        }

        fn search_delete(&self, _: SearchQuery) -> Result<usize, AppError> {
            unimplemented!()
        }

        fn search_update(&self, _: SearchQuery, _: BookmarkUpdate) -> Result<usize, AppError> {
            unimplemented!()
        }

        fn total(&self) -> Result<usize, AppError> {
            Ok(self.bookmarks.len())
        }

        fn tags(&self) -> Result<Vec<String>, AppError> {
            unimplemented!()
        }

        fn search(&self, query: SearchQuery) -> Result<Vec<Bookmark>, AppError> {
            let mut results = self.bookmarks.clone();

            // Apply title filter (substring match)
            if let Some(ref title) = query.title {
                let title_lower = title.to_lowercase();
                results.retain(|b| b.title.to_lowercase().contains(&title_lower));
            }

            // Apply tag filter
            if let Some(ref tags) = query.tags {
                results.retain(|b| tags.iter().all(|t| b.tags.contains(t)));
            }

            // Apply limit
            if let Some(limit) = query.limit {
                results.truncate(limit);
            }

            Ok(results)
        }

        fn config(&self) -> Result<Arc<RwLock<Config>>, AppError> {
            Ok(Arc::new(RwLock::new(Config::default())))
        }

        fn update_config(&self, _: Config) -> Result<(), AppError> {
            unimplemented!()
        }
    }

    /// Test that semantic ranking reorders filtered results by similarity.
    ///
    /// Setup: 4 bookmarks, filter to 3 (exclude cooking), rank by "AI" query.
    /// Expected: ML-related bookmarks should rank higher than web dev.
    #[test]
    #[ignore = "requires model download (~23MB)"]
    fn test_filter_then_rank_orders_by_similarity() {
        let test_dir = test_dir();

        // Create test bookmarks
        let bookmarks = vec![
            create_bookmark(1, "Machine Learning Tutorial", "Introduction to ML algorithms"),
            create_bookmark(2, "Rust Programming", "Systems programming language"),
            create_bookmark(3, "Deep Neural Networks", "Advanced AI and deep learning techniques"),
            create_bookmark(4, "Cooking Recipes", "Best chocolate cake recipes"),
        ];

        // Set up semantic service with embeddings
        let config = enabled_semantic_config();
        let semantic_service = Arc::new(SemanticSearchService::new(config, test_dir.clone()));
        semantic_service.initialize().expect("Failed to initialize");

        // Index all bookmarks
        semantic_service
            .with_index_mut(|index, model| {
                for bm in &bookmarks {
                    if let Some(content) = preprocess_content(&bm.title, &bm.description) {
                        let embedding = model.embed(&content).expect("Failed to embed");
                        let hash = content_hash(&bm.title, &bm.description);
                        index.insert(bm.id, hash, embedding).expect("Failed to insert");
                    }
                }
            })
            .expect("Failed to index");

        // Create service with mock backend (filter) + semantic service (rank)
        let backend = Box::new(FilterableMockBackend::new(bookmarks));
        let service = AppService::with_semantic(backend, semantic_service);

        // Search: no filter, semantic query "artificial intelligence"
        let query = SearchQuery {
            semantic: Some("artificial intelligence deep learning".to_string()),
            ..Default::default()
        };
        let results = service.search_bookmarks(query, false).expect("Search failed");

        // Verify: results are ordered by relevance to AI query
        // ML and Deep Learning bookmarks should come before Rust and Cooking
        assert!(!results.is_empty(), "Expected results");

        // First result should be ML-related (id 1 or 3)
        let first_id = results[0].id;
        assert!(
            first_id == 1 || first_id == 3,
            "Expected ML-related bookmark first, got id={}",
            first_id
        );

        // Verify ML bookmarks rank higher than unrelated content
        let ml_ids: Vec<u64> = results.iter().filter(|b| b.id == 1 || b.id == 3).map(|b| b.id).collect();
        let cooking_pos = results.iter().position(|b| b.id == 4);

        // If cooking is present, it should be after ML bookmarks
        if let Some(cook_idx) = cooking_pos {
            for &ml_id in &ml_ids {
                let ml_pos = results.iter().position(|b| b.id == ml_id).unwrap();
                assert!(
                    ml_pos < cook_idx,
                    "ML bookmark {} should rank before cooking",
                    ml_id
                );
            }
        }

        // Cleanup
        let _ = std::fs::remove_dir_all(&test_dir);
    }

    /// Test that filtering is applied BEFORE semantic ranking.
    ///
    /// Setup: 4 bookmarks, filter by title="programming", then rank.
    /// Expected: Only "Rust Programming" is returned (other bookmarks filtered out).
    #[test]
    #[ignore = "requires model download (~23MB)"]
    fn test_filter_applied_before_ranking() {
        let test_dir = test_dir();

        let bookmarks = vec![
            create_bookmark(1, "Machine Learning Tutorial", "Introduction to ML"),
            create_bookmark(2, "Rust Programming Guide", "Systems programming"),
            create_bookmark(3, "Deep Neural Networks", "Advanced AI"),
            create_bookmark(4, "Cooking Recipes", "Chocolate cake"),
        ];

        let config = enabled_semantic_config();
        let semantic_service = Arc::new(SemanticSearchService::new(config, test_dir.clone()));
        semantic_service.initialize().expect("Failed to initialize");

        semantic_service
            .with_index_mut(|index, model| {
                for bm in &bookmarks {
                    if let Some(content) = preprocess_content(&bm.title, &bm.description) {
                        let embedding = model.embed(&content).expect("Failed to embed");
                        let hash = content_hash(&bm.title, &bm.description);
                        index.insert(bm.id, hash, embedding).expect("Failed to insert");
                    }
                }
            })
            .expect("Failed to index");

        let backend = Box::new(FilterableMockBackend::new(bookmarks));
        let service = AppService::with_semantic(backend, semantic_service);

        // Filter by title + semantic query
        let query = SearchQuery {
            title: Some("programming".to_string()),
            semantic: Some("systems rust cargo".to_string()),
            ..Default::default()
        };
        let results = service.search_bookmarks(query, false).expect("Search failed");

        // Should only get Rust Programming (id=2) - filter happens first
        assert_eq!(results.len(), 1, "Expected exactly 1 result after filter");
        assert_eq!(results[0].id, 2, "Expected Rust Programming bookmark");

        let _ = std::fs::remove_dir_all(&test_dir);
    }

    /// Test that threshold excludes low-similarity results.
    ///
    /// Setup: Bookmarks with varying relevance, high threshold.
    /// Expected: Only highly relevant results pass threshold.
    #[test]
    #[ignore = "requires model download (~23MB)"]
    fn test_threshold_filters_low_similarity() {
        let test_dir = test_dir();

        let bookmarks = vec![
            create_bookmark(1, "Deep Learning with TensorFlow", "Neural networks and AI"),
            create_bookmark(2, "Gardening Tips", "How to grow tomatoes"),
            create_bookmark(3, "Machine Learning Algorithms", "Supervised and unsupervised learning"),
        ];

        let config = enabled_semantic_config();
        let semantic_service = Arc::new(SemanticSearchService::new(config, test_dir.clone()));
        semantic_service.initialize().expect("Failed to initialize");

        semantic_service
            .with_index_mut(|index, model| {
                for bm in &bookmarks {
                    if let Some(content) = preprocess_content(&bm.title, &bm.description) {
                        let embedding = model.embed(&content).expect("Failed to embed");
                        let hash = content_hash(&bm.title, &bm.description);
                        index.insert(bm.id, hash, embedding).expect("Failed to insert");
                    }
                }
            })
            .expect("Failed to index");

        let backend = Box::new(FilterableMockBackend::new(bookmarks));
        let service = AppService::with_semantic(backend, semantic_service);

        // High threshold - only very relevant results
        let query = SearchQuery {
            semantic: Some("artificial intelligence machine learning".to_string()),
            threshold: Some(0.5),
            ..Default::default()
        };
        let results = service.search_bookmarks(query, false).expect("Search failed");

        // Gardening (id=2) should be excluded due to threshold
        for result in &results {
            assert_ne!(
                result.id, 2,
                "Gardening bookmark should be below threshold"
            );
        }

        // At least one ML bookmark should be included
        let ml_count = results.iter().filter(|b| b.id == 1 || b.id == 3).count();
        assert!(ml_count > 0, "At least one ML bookmark should pass threshold");

        let _ = std::fs::remove_dir_all(&test_dir);
    }

    /// Test that limit is respected after semantic ranking.
    ///
    /// Setup: 4 relevant bookmarks, limit=2.
    /// Expected: Only top 2 by similarity returned.
    #[test]
    #[ignore = "requires model download (~23MB)"]
    fn test_limit_applied_after_ranking() {
        let test_dir = test_dir();

        let bookmarks = vec![
            create_bookmark(1, "Machine Learning Guide", "ML introduction"),
            create_bookmark(2, "Deep Learning Tutorial", "Neural networks"),
            create_bookmark(3, "AI Fundamentals", "Artificial intelligence basics"),
            create_bookmark(4, "Cooking Guide", "Kitchen recipes"),
        ];

        let config = enabled_semantic_config();
        let semantic_service = Arc::new(SemanticSearchService::new(config, test_dir.clone()));
        semantic_service.initialize().expect("Failed to initialize");

        semantic_service
            .with_index_mut(|index, model| {
                for bm in &bookmarks {
                    if let Some(content) = preprocess_content(&bm.title, &bm.description) {
                        let embedding = model.embed(&content).expect("Failed to embed");
                        let hash = content_hash(&bm.title, &bm.description);
                        index.insert(bm.id, hash, embedding).expect("Failed to insert");
                    }
                }
            })
            .expect("Failed to index");

        let backend = Box::new(FilterableMockBackend::new(bookmarks));
        let service = AppService::with_semantic(backend, semantic_service);

        // Query with limit=2
        let query = SearchQuery {
            semantic: Some("machine learning AI".to_string()),
            limit: Some(2),
            threshold: Some(0.0), // Low threshold to include all, rely on limit
            ..Default::default()
        };
        let results = service.search_bookmarks(query, false).expect("Search failed");

        // Should return exactly 2 results
        assert_eq!(results.len(), 2, "Expected exactly 2 results with limit=2");

        // Results should be ML-related (not cooking)
        for result in &results {
            assert_ne!(result.id, 4, "Cooking should not be in top 2 for ML query");
        }

        let _ = std::fs::remove_dir_all(&test_dir);
    }

    /// Test that empty filtered results returns empty (no semantic search performed).
    #[test]
    #[ignore = "requires model download (~23MB)"]
    fn test_empty_filter_returns_empty() {
        let test_dir = test_dir();

        let bookmarks = vec![
            create_bookmark(1, "Machine Learning", "ML guide"),
            create_bookmark(2, "Rust Programming", "Systems programming"),
        ];

        let config = enabled_semantic_config();
        let semantic_service = Arc::new(SemanticSearchService::new(config, test_dir.clone()));
        semantic_service.initialize().expect("Failed to initialize");

        semantic_service
            .with_index_mut(|index, model| {
                for bm in &bookmarks {
                    if let Some(content) = preprocess_content(&bm.title, &bm.description) {
                        let embedding = model.embed(&content).expect("Failed to embed");
                        let hash = content_hash(&bm.title, &bm.description);
                        index.insert(bm.id, hash, embedding).expect("Failed to insert");
                    }
                }
            })
            .expect("Failed to index");

        let backend = Box::new(FilterableMockBackend::new(bookmarks));
        let service = AppService::with_semantic(backend, semantic_service);

        // Filter that matches nothing + semantic query
        let query = SearchQuery {
            title: Some("nonexistent title xyz".to_string()),
            semantic: Some("machine learning".to_string()),
            ..Default::default()
        };
        let results = service.search_bookmarks(query, false).expect("Search failed");

        // Should return empty (filter yields nothing)
        assert!(results.is_empty(), "Expected empty results when filter matches nothing");

        let _ = std::fs::remove_dir_all(&test_dir);
    }
}
