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
        let content = preprocess_content(title, description, &[], "").unwrap();
        let embedding = model.embed(&content).expect("Failed to generate embedding");

        let hash = content_hash(title, description, &[], "");
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
    let content = preprocess_content("日本語タイトル", "Unicode description 中文", &[], "");
    assert!(content.is_some());

    // Very long content
    let long_title = "A".repeat(1000);
    let content = preprocess_content(&long_title, "Short", &[], "");
    let content = content.unwrap();
    assert!(content.len() <= 512);
    assert!(content.ends_with("..."));

    // Whitespace only
    assert!(preprocess_content("   ", "\t\n", &[], "").is_none());
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
            download_timeout_secs: 300,
            semantic_weight: 0.6,
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
        fn bookmark_version(&self) -> u64 { 0 }
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
                    if let Some(content) = preprocess_content(&bm.title, &bm.description, &bm.tags, &bm.url) {
                        let embedding = model.embed(&content).expect("Failed to embed");
                        let hash = content_hash(&bm.title, &bm.description, &bm.tags, &bm.url);
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
                    if let Some(content) = preprocess_content(&bm.title, &bm.description, &bm.tags, &bm.url) {
                        let embedding = model.embed(&content).expect("Failed to embed");
                        let hash = content_hash(&bm.title, &bm.description, &bm.tags, &bm.url);
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
                    if let Some(content) = preprocess_content(&bm.title, &bm.description, &bm.tags, &bm.url) {
                        let embedding = model.embed(&content).expect("Failed to embed");
                        let hash = content_hash(&bm.title, &bm.description, &bm.tags, &bm.url);
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
                    if let Some(content) = preprocess_content(&bm.title, &bm.description, &bm.tags, &bm.url) {
                        let embedding = model.embed(&content).expect("Failed to embed");
                        let hash = content_hash(&bm.title, &bm.description, &bm.tags, &bm.url);
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
                    if let Some(content) = preprocess_content(&bm.title, &bm.description, &bm.tags, &bm.url) {
                        let embedding = model.embed(&content).expect("Failed to embed");
                        let hash = content_hash(&bm.title, &bm.description, &bm.tags, &bm.url);
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

    // =========================================================================
    // D.3 Orphan Filtering Tests
    //
    // These tests verify that deleted bookmarks (orphans in the vector index)
    // do not appear in semantic search results.
    // =========================================================================

    /// Test that deleted bookmarks don't appear in semantic search results.
    ///
    /// Scenario: A bookmark is indexed, then deleted from the backend.
    /// The vector index still contains the embedding (orphan), but semantic
    /// search should NOT return it because it's filtered out by the
    /// filter-then-rank flow.
    ///
    /// This is the core acceptance test for D.3: "Deleted bookmarks don't
    /// appear in results."
    #[test]
    #[ignore = "requires model download (~23MB)"]
    fn test_deleted_bookmark_not_in_semantic_results() {
        let test_dir = test_dir();

        // Create 3 bookmarks - all will be indexed
        let all_bookmarks = vec![
            create_bookmark(1, "Machine Learning Tutorial", "Introduction to ML algorithms"),
            create_bookmark(2, "Rust Programming", "Systems programming language"),
            create_bookmark(3, "Deep Neural Networks", "Advanced AI and deep learning"),
        ];

        // But only 2 will be "in the database" (simulating bookmark #2 was deleted)
        let remaining_bookmarks = vec![
            create_bookmark(1, "Machine Learning Tutorial", "Introduction to ML algorithms"),
            create_bookmark(3, "Deep Neural Networks", "Advanced AI and deep learning"),
        ];

        let config = enabled_semantic_config();
        let semantic_service = Arc::new(SemanticSearchService::new(config, test_dir.clone()));
        semantic_service.initialize().expect("Failed to initialize");

        // Index ALL bookmarks (including the one that will be "deleted")
        semantic_service
            .with_index_mut(|index, model| {
                for bm in &all_bookmarks {
                    if let Some(content) = preprocess_content(&bm.title, &bm.description, &bm.tags, &bm.url) {
                        let embedding = model.embed(&content).expect("Failed to embed");
                        let hash = content_hash(&bm.title, &bm.description, &bm.tags, &bm.url);
                        index.insert(bm.id, hash, embedding).expect("Failed to insert");
                    }
                }
            })
            .expect("Failed to index");

        // Verify index has all 3 entries (including orphan)
        assert_eq!(semantic_service.indexed_count(), 3, "Index should have 3 entries");

        // Backend returns only remaining bookmarks (simulates deletion)
        let backend = Box::new(FilterableMockBackend::new(remaining_bookmarks));
        let service = AppService::with_semantic(backend, semantic_service);

        // Search with a query that would match the deleted bookmark
        // "Rust programming" should NOT appear because it's not in backend results
        let query = SearchQuery {
            semantic: Some("systems programming rust".to_string()),
            threshold: Some(0.0), // Low threshold to ensure we'd see it if present
            ..Default::default()
        };
        let results = service.search_bookmarks(query, false).expect("Search failed");

        // CRITICAL ASSERTION: Deleted bookmark (id=2) should NOT be in results
        let ids: Vec<u64> = results.iter().map(|b| b.id).collect();
        assert!(
            !ids.contains(&2),
            "Deleted bookmark (id=2) should NOT appear in results. Got: {:?}",
            ids
        );

        // Only remaining bookmarks should be present
        for result in &results {
            assert!(
                result.id == 1 || result.id == 3,
                "Result should be from remaining bookmarks, got id={}",
                result.id
            );
        }

        let _ = std::fs::remove_dir_all(&test_dir);
    }

    /// Test orphan filtering with semantic-only search (no other filters).
    ///
    /// This is the edge case where candidate_ids comes from ALL backend
    /// bookmarks, not a filtered subset. The orphan should still be excluded.
    #[test]
    #[ignore = "requires model download (~23MB)"]
    fn test_orphan_excluded_with_semantic_only_search() {
        let test_dir = test_dir();

        // 4 bookmarks total, but #4 will be "deleted"
        let all_bookmarks = vec![
            create_bookmark(1, "Python Programming", "Python language tutorial"),
            create_bookmark(2, "JavaScript Guide", "JS for web development"),
            create_bookmark(3, "Go Programming", "Golang systems programming"),
            create_bookmark(4, "Ruby on Rails", "Ruby web framework tutorial"),
        ];

        // #4 is deleted
        let remaining_bookmarks = vec![
            create_bookmark(1, "Python Programming", "Python language tutorial"),
            create_bookmark(2, "JavaScript Guide", "JS for web development"),
            create_bookmark(3, "Go Programming", "Golang systems programming"),
        ];

        let config = enabled_semantic_config();
        let semantic_service = Arc::new(SemanticSearchService::new(config, test_dir.clone()));
        semantic_service.initialize().expect("Failed to initialize");

        // Index all 4 (including future orphan)
        semantic_service
            .with_index_mut(|index, model| {
                for bm in &all_bookmarks {
                    if let Some(content) = preprocess_content(&bm.title, &bm.description, &bm.tags, &bm.url) {
                        let embedding = model.embed(&content).expect("Failed to embed");
                        let hash = content_hash(&bm.title, &bm.description, &bm.tags, &bm.url);
                        index.insert(bm.id, hash, embedding).expect("Failed to insert");
                    }
                }
            })
            .expect("Failed to index");

        let backend = Box::new(FilterableMockBackend::new(remaining_bookmarks));
        let service = AppService::with_semantic(backend, semantic_service);

        // Semantic-only search with query matching the deleted bookmark
        let query = SearchQuery {
            semantic: Some("ruby rails web framework".to_string()),
            threshold: Some(0.0),
            ..Default::default()
        };
        let results = service.search_bookmarks(query, false).expect("Search failed");

        // The deleted Ruby bookmark should NOT be in results
        let ids: Vec<u64> = results.iter().map(|b| b.id).collect();
        assert!(
            !ids.contains(&4),
            "Orphan bookmark (id=4, Ruby) should NOT appear. Got: {:?}",
            ids
        );

        // Results should only contain IDs 1, 2, or 3
        for result in &results {
            assert!(
                result.id <= 3,
                "Only remaining bookmarks should appear, got id={}",
                result.id
            );
        }

        let _ = std::fs::remove_dir_all(&test_dir);
    }

    /// Test that multiple orphans are all filtered out.
    #[test]
    #[ignore = "requires model download (~23MB)"]
    fn test_multiple_orphans_filtered() {
        let test_dir = test_dir();

        // 5 bookmarks, but only 2 remain
        let all_bookmarks = vec![
            create_bookmark(1, "Staying Bookmark", "This one stays"),
            create_bookmark(2, "Deleted One", "Will be orphaned"),
            create_bookmark(3, "Another Keeper", "This also stays"),
            create_bookmark(4, "Deleted Two", "Also orphaned"),
            create_bookmark(5, "Deleted Three", "Orphaned too"),
        ];

        let remaining_bookmarks = vec![
            create_bookmark(1, "Staying Bookmark", "This one stays"),
            create_bookmark(3, "Another Keeper", "This also stays"),
        ];

        let config = enabled_semantic_config();
        let semantic_service = Arc::new(SemanticSearchService::new(config, test_dir.clone()));
        semantic_service.initialize().expect("Failed to initialize");

        semantic_service
            .with_index_mut(|index, model| {
                for bm in &all_bookmarks {
                    if let Some(content) = preprocess_content(&bm.title, &bm.description, &bm.tags, &bm.url) {
                        let embedding = model.embed(&content).expect("Failed to embed");
                        let hash = content_hash(&bm.title, &bm.description, &bm.tags, &bm.url);
                        index.insert(bm.id, hash, embedding).expect("Failed to insert");
                    }
                }
            })
            .expect("Failed to index");

        // Index has 5, backend has 2
        assert_eq!(semantic_service.indexed_count(), 5);

        let backend = Box::new(FilterableMockBackend::new(remaining_bookmarks));
        let service = AppService::with_semantic(backend, semantic_service);

        let query = SearchQuery {
            semantic: Some("bookmark".to_string()),
            threshold: Some(0.0),
            ..Default::default()
        };
        let results = service.search_bookmarks(query, false).expect("Search failed");

        // All 3 orphans (2, 4, 5) should be excluded
        let ids: Vec<u64> = results.iter().map(|b| b.id).collect();
        assert!(
            !ids.contains(&2) && !ids.contains(&4) && !ids.contains(&5),
            "Orphans should be excluded. Got: {:?}",
            ids
        );

        // Only valid IDs should appear
        assert!(
            ids.iter().all(|&id| id == 1 || id == 3),
            "Only remaining bookmarks should appear. Got: {:?}",
            ids
        );

        let _ = std::fs::remove_dir_all(&test_dir);
    }
}

// =============================================================================
// D.1 Index Maintenance Tests: Embedding on Bookmark Create
//
// These tests verify that new bookmarks are automatically embedded and indexed
// for semantic search when created through AppService.
// =============================================================================

mod index_maintenance {
    use crate::app::backend::{AddOpts, AppBackend, RefreshMetadataOpts};
    use crate::app::errors::AppError;
    use crate::app::service::AppService;
    use crate::bookmarks::{Bookmark, BookmarkCreate, BookmarkUpdate, SearchQuery};
    use crate::config::{Config, SemanticSearchConfig};
    use crate::semantic::{content_hash, SemanticSearchService};
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::sync::{Arc, RwLock};

    static TEST_COUNTER: AtomicU64 = AtomicU64::new(0);

    fn test_dir() -> PathBuf {
        let counter = TEST_COUNTER.fetch_add(1, Ordering::SeqCst);
        let path = std::env::temp_dir().join(format!(
            "bb-index-maintenance-{}-{}",
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
            download_timeout_secs: 300,
            semantic_weight: 0.6,
        }
    }

    /// Mock backend that supports create and tracks created bookmarks.
    struct CreateMockBackend {
        bookmarks: RwLock<Vec<Bookmark>>,
        next_id: AtomicU64,
    }

    impl CreateMockBackend {
        fn new() -> Self {
            Self {
                bookmarks: RwLock::new(Vec::new()),
                next_id: AtomicU64::new(1),
            }
        }
    }

    impl AppBackend for CreateMockBackend {
        fn create(&self, create: BookmarkCreate, _: AddOpts) -> Result<Bookmark, AppError> {
            let id = self.next_id.fetch_add(1, Ordering::SeqCst);

            let bookmark = Bookmark {
                id,
                url: create.url,
                title: create.title.unwrap_or_default(),
                description: create.description.unwrap_or_default(),
                tags: create.tags.unwrap_or_default(),
                image_id: None,
                icon_id: None,
            };

            self.bookmarks.write().unwrap().push(bookmark.clone());
            Ok(bookmark)
        }

        fn search(&self, _: SearchQuery) -> Result<Vec<Bookmark>, AppError> {
            // Return empty for duplicate check
            Ok(Vec::new())
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
            Ok(self.bookmarks.read().unwrap().len())
        }

        fn tags(&self) -> Result<Vec<String>, AppError> {
            unimplemented!()
        }

        fn config(&self) -> Result<Arc<RwLock<Config>>, AppError> {
            Ok(Arc::new(RwLock::new(Config::default())))
        }

        fn update_config(&self, _: Config) -> Result<(), AppError> {
            unimplemented!()
        }
        fn bookmark_version(&self) -> u64 { 0 }
    }

    /// Test that creating a bookmark indexes it for semantic search.
    ///
    /// This is the core acceptance test for D.1: "New bookmark appears in
    /// semantic search results."
    #[test]
    #[ignore = "requires model download (~23MB)"]
    fn test_create_bookmark_indexes_for_semantic_search() {
        let test_dir = test_dir();

        let config = enabled_semantic_config();
        let semantic_service = Arc::new(SemanticSearchService::new(config, test_dir.clone()));

        // Initialize service (model load happens here)
        semantic_service.initialize().expect("Failed to initialize");
        assert_eq!(semantic_service.indexed_count(), 0, "Should start empty");

        let backend = Box::new(CreateMockBackend::new());
        let service = AppService::with_semantic(backend, semantic_service.clone());

        // Create a bookmark
        let create = BookmarkCreate {
            url: "https://example.com/ml-guide".to_string(),
            title: Some("Machine Learning Tutorial".to_string()),
            description: Some("Introduction to neural networks and deep learning".to_string()),
            tags: None,
            image_id: None,
            icon_id: None,
        };
        let opts = AddOpts::default();

        let bookmark = service.create_bookmark(create, opts).expect("Create failed");
        assert_eq!(bookmark.id, 1);

        // Verify: bookmark was indexed
        assert_eq!(
            semantic_service.indexed_count(),
            1,
            "Bookmark should be indexed after create"
        );

        // Verify: can be found via semantic search
        let results = semantic_service
            .search("artificial intelligence neural networks", None, Some(0.3), 10)
            .expect("Search failed");

        assert!(
            results.contains(&1),
            "Created bookmark should appear in semantic search. Got: {:?}",
            results
        );

        let _ = std::fs::remove_dir_all(&test_dir);
    }

    /// Test that creating multiple bookmarks indexes all of them.
    #[test]
    #[ignore = "requires model download (~23MB)"]
    fn test_multiple_creates_all_indexed() {
        let test_dir = test_dir();

        let config = enabled_semantic_config();
        let semantic_service = Arc::new(SemanticSearchService::new(config, test_dir.clone()));
        semantic_service.initialize().expect("Failed to initialize");

        let backend = Box::new(CreateMockBackend::new());
        let service = AppService::with_semantic(backend, semantic_service.clone());

        let opts = AddOpts::default();

        // Create several bookmarks
        let creates = vec![
            BookmarkCreate {
                url: "https://example.com/rust".to_string(),
                title: Some("Rust Programming".to_string()),
                description: Some("Systems programming language".to_string()),
                tags: None,
                image_id: None,
                icon_id: None,
            },
            BookmarkCreate {
                url: "https://example.com/python".to_string(),
                title: Some("Python Tutorial".to_string()),
                description: Some("Data science and machine learning".to_string()),
                tags: None,
                image_id: None,
                icon_id: None,
            },
            BookmarkCreate {
                url: "https://example.com/cooking".to_string(),
                title: Some("Best Recipes".to_string()),
                description: Some("Chocolate cake and desserts".to_string()),
                tags: None,
                image_id: None,
                icon_id: None,
            },
        ];

        for create in creates {
            service
                .create_bookmark(create, opts.clone())
                .expect("Create failed");
        }

        // Verify: all 3 indexed
        assert_eq!(
            semantic_service.indexed_count(),
            3,
            "All bookmarks should be indexed"
        );

        let _ = std::fs::remove_dir_all(&test_dir);
    }

    /// Test that empty content (no title, no description) is skipped.
    #[test]
    #[ignore = "requires model download (~23MB)"]
    fn test_create_empty_content_skips_embedding() {
        let test_dir = test_dir();

        let config = enabled_semantic_config();
        let semantic_service = Arc::new(SemanticSearchService::new(config, test_dir.clone()));
        semantic_service.initialize().expect("Failed to initialize");

        let backend = Box::new(CreateMockBackend::new());
        let service = AppService::with_semantic(backend, semantic_service.clone());

        let opts = AddOpts::default();

        // Create bookmark with no title or description
        let create = BookmarkCreate {
            url: "https://example.com/empty".to_string(),
            title: None,
            description: None,
            tags: None,
            image_id: None,
            icon_id: None,
        };

        let bookmark = service.create_bookmark(create, opts).expect("Create failed");
        assert_eq!(bookmark.id, 1);

        // Verify: bookmark was NOT indexed (no content to embed)
        assert_eq!(
            semantic_service.indexed_count(),
            0,
            "Empty content should not be indexed"
        );

        let _ = std::fs::remove_dir_all(&test_dir);
    }

    /// Test that create works without semantic service (no panic).
    #[test]
    fn test_create_without_semantic_service() {
        let backend = Box::new(CreateMockBackend::new());
        let service = AppService::new(backend);

        let create = BookmarkCreate {
            url: "https://example.com/test".to_string(),
            title: Some("Test Bookmark".to_string()),
            description: Some("Test description".to_string()),
            tags: None,
            image_id: None,
            icon_id: None,
        };
        let opts = AddOpts::default();

        // Should succeed without panic
        let result = service.create_bookmark(create, opts);
        assert!(result.is_ok(), "Create should succeed without semantic service");
    }

    /// Test that create works when semantic search is disabled.
    #[test]
    fn test_create_with_semantic_disabled() {
        let test_dir = test_dir();

        let config = SemanticSearchConfig {
            enabled: false,
            ..enabled_semantic_config()
        };
        let semantic_service = Arc::new(SemanticSearchService::new(config, test_dir.clone()));

        let backend = Box::new(CreateMockBackend::new());
        let service = AppService::with_semantic(backend, semantic_service.clone());

        let create = BookmarkCreate {
            url: "https://example.com/test".to_string(),
            title: Some("Test Bookmark".to_string()),
            description: Some("Test description".to_string()),
            tags: None,
            image_id: None,
            icon_id: None,
        };
        let opts = AddOpts::default();

        // Should succeed, but no indexing happens
        let result = service.create_bookmark(create, opts);
        assert!(result.is_ok(), "Create should succeed when disabled");

        // Index should remain empty
        assert_eq!(
            semantic_service.indexed_count(),
            0,
            "Should not index when disabled"
        );

        let _ = std::fs::remove_dir_all(&test_dir);
    }

    /// Test that index is persisted after create.
    #[test]
    #[ignore = "requires model download (~23MB)"]
    fn test_create_persists_to_storage() {
        let test_dir = test_dir();
        let vectors_path = test_dir.join("vectors.bin");

        // Create bookmark with first service instance
        {
            let config = enabled_semantic_config();
            let semantic_service = Arc::new(SemanticSearchService::new(config, test_dir.clone()));
            semantic_service.initialize().expect("Failed to initialize");

            let backend = Box::new(CreateMockBackend::new());
            let service = AppService::with_semantic(backend, semantic_service);

            let create = BookmarkCreate {
                url: "https://example.com/persist".to_string(),
                title: Some("Persisted Bookmark".to_string()),
                description: Some("Should survive reload".to_string()),
                tags: None,
                image_id: None,
                icon_id: None,
            };
            let opts = AddOpts::default();

            service.create_bookmark(create, opts).expect("Create failed");
        }

        // Verify vectors.bin exists
        assert!(vectors_path.exists(), "vectors.bin should be created");

        // Create new service and verify data persists
        {
            let config = enabled_semantic_config();
            let semantic_service = Arc::new(SemanticSearchService::new(config, test_dir.clone()));
            semantic_service.initialize().expect("Failed to initialize");

            assert_eq!(
                semantic_service.indexed_count(),
                1,
                "Index should persist across service restarts"
            );
        }

        let _ = std::fs::remove_dir_all(&test_dir);
    }

    // =========================================================================
    // D.2 Update Revalidation Tests
    //
    // These tests verify that bookmark updates properly revalidate the
    // semantic index entry when content changes.
    // =========================================================================

    /// Mock backend that supports both create and update operations.
    struct UpdateableMockBackend {
        bookmarks: RwLock<Vec<Bookmark>>,
        next_id: AtomicU64,
    }

    impl UpdateableMockBackend {
        fn new() -> Self {
            Self {
                bookmarks: RwLock::new(Vec::new()),
                next_id: AtomicU64::new(1),
            }
        }

        fn with_bookmarks(bookmarks: Vec<Bookmark>) -> Self {
            let max_id = bookmarks.iter().map(|b| b.id).max().unwrap_or(0);
            Self {
                bookmarks: RwLock::new(bookmarks),
                next_id: AtomicU64::new(max_id + 1),
            }
        }
    }

    impl AppBackend for UpdateableMockBackend {
        fn create(&self, create: BookmarkCreate, _: AddOpts) -> Result<Bookmark, AppError> {
            let id = self.next_id.fetch_add(1, Ordering::SeqCst);

            let bookmark = Bookmark {
                id,
                url: create.url,
                title: create.title.unwrap_or_default(),
                description: create.description.unwrap_or_default(),
                tags: create.tags.unwrap_or_default(),
                image_id: None,
                icon_id: None,
            };

            self.bookmarks.write().unwrap().push(bookmark.clone());
            Ok(bookmark)
        }

        fn update(&self, id: u64, update: BookmarkUpdate) -> Result<Bookmark, AppError> {
            let mut bookmarks = self.bookmarks.write().unwrap();
            let bookmark = bookmarks
                .iter_mut()
                .find(|b| b.id == id)
                .ok_or_else(|| AppError::Other(anyhow::anyhow!("Bookmark {} not found", id)))?;

            if let Some(title) = update.title {
                bookmark.title = title;
            }
            if let Some(description) = update.description {
                bookmark.description = description;
            }
            if let Some(url) = update.url {
                bookmark.url = url;
            }
            if let Some(tags) = update.tags {
                bookmark.tags = tags;
            }

            Ok(bookmark.clone())
        }

        fn search(&self, query: SearchQuery) -> Result<Vec<Bookmark>, AppError> {
            let bookmarks = self.bookmarks.read().unwrap();
            if let Some(id) = query.id {
                Ok(bookmarks.iter().filter(|b| b.id == id).cloned().collect())
            } else if query.url.is_some() && query.exact {
                // For duplicate check - return empty (no duplicates)
                Ok(Vec::new())
            } else {
                Ok(bookmarks.clone())
            }
        }

        fn refresh_metadata(&self, _: u64, _: RefreshMetadataOpts) -> Result<(), AppError> {
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
            Ok(self.bookmarks.read().unwrap().len())
        }

        fn tags(&self) -> Result<Vec<String>, AppError> {
            unimplemented!()
        }

        fn config(&self) -> Result<Arc<RwLock<Config>>, AppError> {
            Ok(Arc::new(RwLock::new(Config::default())))
        }

        fn update_config(&self, _: Config) -> Result<(), AppError> {
            unimplemented!()
        }
        fn bookmark_version(&self) -> u64 { 0 }
    }

    /// Test that updating bookmark content triggers re-embedding.
    ///
    /// This is the core acceptance test for D.2: "Updated bookmark reflects
    /// new content in semantic search."
    #[test]
    #[ignore = "requires model download (~23MB)"]
    fn test_update_content_triggers_reembed() {
        use crate::semantic::content_hash;

        let test_dir = test_dir();

        let config = enabled_semantic_config();
        let semantic_service = Arc::new(SemanticSearchService::new(config, test_dir.clone()));
        semantic_service.initialize().expect("Failed to initialize");

        let backend = Box::new(UpdateableMockBackend::new());
        let service = AppService::with_semantic(backend, semantic_service.clone());

        // Create a bookmark about cooking
        let create = BookmarkCreate {
            url: "https://example.com/topic".to_string(),
            title: Some("Cooking Recipes".to_string()),
            description: Some("Best chocolate cake recipes".to_string()),
            tags: None,
            image_id: None,
            icon_id: None,
        };
        let bookmark = service
            .create_bookmark(create, AddOpts::default())
            .expect("Create failed");
        let id = bookmark.id;

        // Get original hash
        let original_hash = content_hash(&bookmark.title, &bookmark.description, &bookmark.tags, &bookmark.url);

        // Verify: finds cooking content
        let cooking_results = semantic_service
            .search("chocolate desserts baking", None, Some(0.3), 10)
            .expect("Search failed");
        assert!(
            cooking_results.contains(&id),
            "Should find cooking content before update"
        );

        // Update to machine learning content
        let update = BookmarkUpdate {
            title: Some("Machine Learning Guide".to_string()),
            description: Some("Neural networks and deep learning".to_string()),
            ..Default::default()
        };
        let updated = service.update_bookmark(id, update).expect("Update failed");

        // Verify: hash changed
        let new_hash = content_hash(&updated.title, &updated.description, &updated.tags, &updated.url);
        assert_ne!(original_hash, new_hash, "Content hash should change after update");

        // Verify: now finds ML content
        let ml_results = semantic_service
            .search("artificial intelligence neural networks", None, Some(0.3), 10)
            .expect("Search failed");
        assert!(
            ml_results.contains(&id),
            "Should find ML content after update. Got: {:?}",
            ml_results
        );

        let _ = std::fs::remove_dir_all(&test_dir);
    }

    /// Test that updating tags/URL DOES trigger re-embed (they're part of embedded content).
    #[test]
    #[ignore = "requires model download (~23MB)"]
    fn test_update_tags_url_triggers_reembed() {
        use crate::semantic::content_hash;

        let test_dir = test_dir();

        let config = enabled_semantic_config();
        let semantic_service = Arc::new(SemanticSearchService::new(config, test_dir.clone()));
        semantic_service.initialize().expect("Failed to initialize");

        let backend = Box::new(UpdateableMockBackend::new());
        let service = AppService::with_semantic(backend, semantic_service.clone());

        // Create a bookmark
        let create = BookmarkCreate {
            url: "https://example.com/original".to_string(),
            title: Some("Test Bookmark".to_string()),
            description: Some("Test description".to_string()),
            tags: None,
            image_id: None,
            icon_id: None,
        };
        let bookmark = service
            .create_bookmark(create, AddOpts::default())
            .expect("Create failed");
        let id = bookmark.id;

        // Get hash before update
        let hash_before = content_hash(&bookmark.title, &bookmark.description, &bookmark.tags, &bookmark.url);

        // Update URL and tags (these are now part of embedded content)
        let update = BookmarkUpdate {
            url: Some("https://example.com/new-url".to_string()),
            tags: Some(vec!["new-tag".to_string()]),
            ..Default::default()
        };
        let updated = service.update_bookmark(id, update).expect("Update failed");

        // Hash SHOULD change because tags and URL are part of embedded content
        let hash_after = content_hash(&updated.title, &updated.description, &updated.tags, &updated.url);
        assert_ne!(hash_before, hash_after, "Hash should change when tags/URL are updated");

        let _ = std::fs::remove_dir_all(&test_dir);
    }

    /// Test that update works without semantic service (no panic).
    #[test]
    fn test_update_without_semantic_service() {
        let bookmark = Bookmark {
            id: 1,
            url: "https://example.com/test".to_string(),
            title: "Original Title".to_string(),
            description: "Original description".to_string(),
            tags: vec![],
            image_id: None,
            icon_id: None,
        };

        let backend = Box::new(UpdateableMockBackend::with_bookmarks(vec![bookmark]));
        let service = AppService::new(backend);

        let update = BookmarkUpdate {
            title: Some("Updated Title".to_string()),
            ..Default::default()
        };

        // Should succeed without panic
        let result = service.update_bookmark(1, update);
        assert!(result.is_ok(), "Update should succeed without semantic service");
    }

    /// Test that update works when semantic search is disabled.
    #[test]
    fn test_update_with_semantic_disabled() {
        let test_dir = test_dir();

        let bookmark = Bookmark {
            id: 1,
            url: "https://example.com/test".to_string(),
            title: "Original Title".to_string(),
            description: "Original description".to_string(),
            tags: vec![],
            image_id: None,
            icon_id: None,
        };

        let config = SemanticSearchConfig {
            enabled: false,
            ..enabled_semantic_config()
        };
        let semantic_service = Arc::new(SemanticSearchService::new(config, test_dir.clone()));

        let backend = Box::new(UpdateableMockBackend::with_bookmarks(vec![bookmark]));
        let service = AppService::with_semantic(backend, semantic_service.clone());

        let update = BookmarkUpdate {
            title: Some("Updated Title".to_string()),
            description: Some("Updated description".to_string()),
            ..Default::default()
        };

        // Should succeed, but no indexing happens
        let result = service.update_bookmark(1, update);
        assert!(result.is_ok(), "Update should succeed when disabled");

        // Index should remain empty
        assert_eq!(
            semantic_service.indexed_count(),
            0,
            "Should not index when disabled"
        );

        let _ = std::fs::remove_dir_all(&test_dir);
    }

    /// Test that updating content to empty removes from index.
    #[test]
    #[ignore = "requires model download (~23MB)"]
    fn test_update_to_empty_removes_from_index() {
        let test_dir = test_dir();

        let config = enabled_semantic_config();
        let semantic_service = Arc::new(SemanticSearchService::new(config, test_dir.clone()));
        semantic_service.initialize().expect("Failed to initialize");

        let backend = Box::new(UpdateableMockBackend::new());
        let service = AppService::with_semantic(backend, semantic_service.clone());

        // Create a bookmark with content
        let create = BookmarkCreate {
            url: "https://example.com/content".to_string(),
            title: Some("Test Bookmark".to_string()),
            description: Some("Test description".to_string()),
            tags: None,
            image_id: None,
            icon_id: None,
        };
        service
            .create_bookmark(create, AddOpts::default())
            .expect("Create failed");

        assert_eq!(semantic_service.indexed_count(), 1, "Should be indexed after create");

        // Update to empty content
        let update = BookmarkUpdate {
            title: Some(String::new()),
            description: Some(String::new()),
            ..Default::default()
        };
        service.update_bookmark(1, update).expect("Update failed");

        // Should be removed from index
        assert_eq!(
            semantic_service.indexed_count(),
            0,
            "Should be removed from index when content becomes empty"
        );

        let _ = std::fs::remove_dir_all(&test_dir);
    }

    /// Test that update re-embeds missing entry (not previously indexed).
    #[test]
    #[ignore = "requires model download (~23MB)"]
    fn test_update_indexes_missing_entry() {
        let test_dir = test_dir();

        let config = enabled_semantic_config();
        let semantic_service = Arc::new(SemanticSearchService::new(config, test_dir.clone()));
        semantic_service.initialize().expect("Failed to initialize");

        // Bookmark exists in backend but NOT in semantic index
        let bookmark = Bookmark {
            id: 42,
            url: "https://example.com/orphan".to_string(),
            title: "Orphan Bookmark".to_string(),
            description: "Not indexed yet".to_string(),
            tags: vec![],
            image_id: None,
            icon_id: None,
        };

        let backend = Box::new(UpdateableMockBackend::with_bookmarks(vec![bookmark]));
        let service = AppService::with_semantic(backend, semantic_service.clone());

        assert_eq!(semantic_service.indexed_count(), 0, "Should start empty");

        // Update the bookmark - should trigger indexing since entry is missing
        let update = BookmarkUpdate {
            title: Some("Updated Orphan".to_string()),
            ..Default::default()
        };
        service.update_bookmark(42, update).expect("Update failed");

        // Should now be indexed
        assert_eq!(
            semantic_service.indexed_count(),
            1,
            "Missing entry should be indexed on update"
        );

        let _ = std::fs::remove_dir_all(&test_dir);
    }

    // =========================================================================
    // D.4 Index Reconciliation Tests
    //
    // These tests verify that index reconciliation properly syncs the vector
    // index with the current bookmark state on first semantic search.
    // =========================================================================

    /// Test that reconciliation removes orphan entries (embeddings for deleted bookmarks).
    #[test]
    #[ignore = "requires model download (~23MB)"]
    fn test_reconcile_removes_orphans() {
        let test_dir = test_dir();

        let config = enabled_semantic_config();
        let semantic_service = Arc::new(SemanticSearchService::new(config, test_dir.clone()));
        semantic_service.initialize().expect("Failed to initialize");

        // Pre-populate index with an embedding for a "deleted" bookmark
        semantic_service
            .with_index_mut(|index, model| {
                let emb = model.embed("Orphan content to be removed").unwrap();
                index.insert(999, 12345, emb).unwrap();
            })
            .unwrap();
        semantic_service.save_index().unwrap();

        assert_eq!(semantic_service.indexed_count(), 1, "Should have orphan entry");

        // Backend has NO bookmarks (orphan was "deleted")
        let backend = Box::new(UpdateableMockBackend::new());
        let service = AppService::with_semantic(backend, semantic_service.clone());

        // Trigger reconciliation via semantic search
        let query = SearchQuery {
            semantic: Some("test query".to_string()),
            ..Default::default()
        };
        let _ = service.search_bookmarks(query, false);

        // Orphan should be removed
        assert_eq!(
            semantic_service.indexed_count(),
            0,
            "Orphan entry should be removed after reconciliation"
        );

        let _ = std::fs::remove_dir_all(&test_dir);
    }

    /// Test that reconciliation re-embeds stale entries (content hash mismatch).
    #[test]
    #[ignore = "requires model download (~23MB)"]
    fn test_reconcile_reembeds_stale() {
        let test_dir = test_dir();

        let config = enabled_semantic_config();
        let semantic_service = Arc::new(SemanticSearchService::new(config, test_dir.clone()));
        semantic_service.initialize().expect("Failed to initialize");

        // Pre-populate index with old content (wrong hash)
        semantic_service
            .with_index_mut(|index, model| {
                let emb = model.embed("Old content").unwrap();
                // Use a fake hash that won't match the actual bookmark content
                index.insert(1, 99999, emb).unwrap();
            })
            .unwrap();

        // Bookmark has DIFFERENT content (simulates manual CSV edit)
        let bookmark = Bookmark {
            id: 1,
            url: "https://example.com/stale".to_string(),
            title: "Updated Title".to_string(),
            description: "New description after edit".to_string(),
            tags: vec![],
            image_id: None,
            icon_id: None,
        };

        let backend = Box::new(UpdateableMockBackend::with_bookmarks(vec![bookmark.clone()]));
        let service = AppService::with_semantic(backend, semantic_service.clone());

        // Verify the hash in the index doesn't match
        let expected_hash = content_hash(&bookmark.title, &bookmark.description, &bookmark.tags, &bookmark.url);
        let pre_reconcile_hash = semantic_service
            .with_index_mut(|index, _| index.get(1).unwrap().content_hash)
            .unwrap();
        assert_ne!(
            pre_reconcile_hash, expected_hash,
            "Pre-condition: hash should not match"
        );

        // Trigger reconciliation via semantic search
        let query = SearchQuery {
            semantic: Some("test query".to_string()),
            ..Default::default()
        };
        let _ = service.search_bookmarks(query, false);

        // Hash should now match (re-embedded)
        let post_reconcile_hash = semantic_service
            .with_index_mut(|index, _| index.get(1).unwrap().content_hash)
            .unwrap();
        assert_eq!(
            post_reconcile_hash, expected_hash,
            "Stale entry should be re-embedded with correct hash"
        );

        let _ = std::fs::remove_dir_all(&test_dir);
    }

    /// Test that reconciliation embeds missing entries (bookmark exists but not indexed).
    #[test]
    #[ignore = "requires model download (~23MB)"]
    fn test_reconcile_embeds_missing() {
        let test_dir = test_dir();

        let config = enabled_semantic_config();
        let semantic_service = Arc::new(SemanticSearchService::new(config, test_dir.clone()));
        semantic_service.initialize().expect("Failed to initialize");

        assert_eq!(semantic_service.indexed_count(), 0, "Should start empty");

        // Bookmarks exist but are not indexed (simulates enabling semantic on existing DB)
        let bookmarks = vec![
            Bookmark {
                id: 1,
                url: "https://example.com/a".to_string(),
                title: "Machine Learning Tutorial".to_string(),
                description: "Introduction to ML".to_string(),
                tags: vec![],
                image_id: None,
                icon_id: None,
            },
            Bookmark {
                id: 2,
                url: "https://example.com/b".to_string(),
                title: "Cooking Recipes".to_string(),
                description: "Delicious food ideas".to_string(),
                tags: vec![],
                image_id: None,
                icon_id: None,
            },
        ];

        let backend = Box::new(UpdateableMockBackend::with_bookmarks(bookmarks));
        let service = AppService::with_semantic(backend, semantic_service.clone());

        // Trigger reconciliation via semantic search
        let query = SearchQuery {
            semantic: Some("machine learning".to_string()),
            ..Default::default()
        };
        let _ = service.search_bookmarks(query, false);

        // Both bookmarks should now be indexed
        assert_eq!(
            semantic_service.indexed_count(),
            2,
            "Missing entries should be embedded after reconciliation"
        );

        let _ = std::fs::remove_dir_all(&test_dir);
    }

    /// Test that reconciliation is idempotent (second call is a no-op).
    #[test]
    #[ignore = "requires model download (~23MB)"]
    fn test_reconcile_idempotent() {
        let test_dir = test_dir();

        let config = enabled_semantic_config();
        let semantic_service = Arc::new(SemanticSearchService::new(config, test_dir.clone()));
        semantic_service.initialize().expect("Failed to initialize");

        let bookmark = Bookmark {
            id: 1,
            url: "https://example.com/test".to_string(),
            title: "Test Bookmark".to_string(),
            description: "Test description".to_string(),
            tags: vec![],
            image_id: None,
            icon_id: None,
        };

        let backend = Box::new(UpdateableMockBackend::with_bookmarks(vec![bookmark]));
        let service = AppService::with_semantic(backend, semantic_service.clone());

        assert!(!semantic_service.is_reconciled(), "Should not be reconciled yet");

        // First search triggers reconciliation
        let query = SearchQuery {
            semantic: Some("test".to_string()),
            ..Default::default()
        };
        let _ = service.search_bookmarks(query.clone(), false);

        assert!(semantic_service.is_reconciled(), "Should be reconciled after first search");
        assert_eq!(semantic_service.indexed_count(), 1);

        // Second search should NOT trigger reconciliation again
        // (We can't easily verify no work was done, but we verify state is unchanged)
        let _ = service.search_bookmarks(query, false);

        assert!(semantic_service.is_reconciled(), "Should remain reconciled");
        assert_eq!(semantic_service.indexed_count(), 1, "Count should be unchanged");

        let _ = std::fs::remove_dir_all(&test_dir);
    }

    /// Test reconciliation without model (unit test that can run fast).
    /// Verifies the service API contract without requiring model download.
    #[test]
    fn test_reconcile_disabled_returns_error() {
        let test_dir = test_dir();

        let config = SemanticSearchConfig {
            enabled: false,
            ..enabled_semantic_config()
        };
        let semantic_service = Arc::new(SemanticSearchService::new(config, test_dir.clone()));

        let result = semantic_service.reconcile(&[]);
        assert!(
            matches!(result, Err(crate::semantic::SemanticSearchError::Disabled)),
            "Reconcile should fail when disabled"
        );

        let _ = std::fs::remove_dir_all(&test_dir);
    }

    /// Test that empty bookmarks with no content are skipped during reconciliation.
    #[test]
    #[ignore = "requires model download (~23MB)"]
    fn test_reconcile_skips_empty_content() {
        let test_dir = test_dir();

        let config = enabled_semantic_config();
        let semantic_service = Arc::new(SemanticSearchService::new(config, test_dir.clone()));
        semantic_service.initialize().expect("Failed to initialize");

        // Bookmark with no title or description (can't be embedded)
        let bookmark = Bookmark {
            id: 1,
            url: "https://example.com/empty".to_string(),
            title: "".to_string(),
            description: "".to_string(),
            tags: vec!["tag".to_string()],
            image_id: None,
            icon_id: None,
        };

        let backend = Box::new(UpdateableMockBackend::with_bookmarks(vec![bookmark]));
        let service = AppService::with_semantic(backend, semantic_service.clone());

        // Trigger reconciliation
        let query = SearchQuery {
            semantic: Some("test".to_string()),
            ..Default::default()
        };
        let _ = service.search_bookmarks(query, false);

        // Empty content bookmark should not be indexed
        assert_eq!(
            semantic_service.indexed_count(),
            0,
            "Empty content bookmarks should not be indexed"
        );

        let _ = std::fs::remove_dir_all(&test_dir);
    }
}
