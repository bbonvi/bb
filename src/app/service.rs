use crate::{
    app::backend::{AddOpts, AppBackend, RefreshMetadataOpts},
    bookmarks::{Bookmark, BookmarkCreate, BookmarkUpdate, SearchQuery},
    config::Config,
    semantic::{content_hash, preprocess_content, SemanticSearchService},
};
use anyhow::{Context, Result};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};

/// Application service layer that provides business logic and orchestrates operations
pub struct AppService {
    backend: Box<dyn AppBackend>,
    /// Semantic search service (None for remote backend mode)
    semantic_service: Option<Arc<SemanticSearchService>>,
}

impl AppService {
    /// Create a new application service with the given backend
    pub fn new(backend: Box<dyn AppBackend>) -> Self {
        Self {
            backend,
            semantic_service: None,
        }
    }

    /// Create a new application service with semantic search support
    pub fn with_semantic(
        backend: Box<dyn AppBackend>,
        semantic_service: Arc<SemanticSearchService>,
    ) -> Self {
        Self {
            backend,
            semantic_service: Some(semantic_service),
        }
    }

    /// Get a reference to the semantic search service (if available)
    pub fn semantic_service(&self) -> Option<&Arc<SemanticSearchService>> {
        self.semantic_service.as_ref()
    }

    // MARK: - Bookmark Operations

    /// Search bookmarks with optional count-only mode
    ///
    /// When `query.semantic` is provided and semantic search is enabled:
    /// 1. First applies all filters (id, title, url, tags, keyword, etc.)
    /// 2. Then ranks filtered results by semantic similarity
    /// 3. Returns results ordered by relevance (highest similarity first)
    pub fn search_bookmarks(&self, query: SearchQuery, count_only: bool) -> Result<Vec<Bookmark>> {
        // Extract semantic params before passing query to backend
        let semantic_query = query.semantic.clone();
        let semantic_threshold = query.threshold;
        let query_limit = query.limit;

        // Apply all filters via backend search
        let mut bookmarks = self
            .backend
            .search(query)
            .context("Failed to search bookmarks")?;

        // If semantic search requested and service available, rank results
        if let Some(ref semantic_text) = semantic_query {
            if let Some(ref service) = self.semantic_service {
                if service.is_enabled() {
                    // Ensure index is reconciled before semantic search
                    self.ensure_index_reconciled(service);

                    bookmarks = self.apply_semantic_ranking(
                        bookmarks,
                        semantic_text,
                        semantic_threshold,
                        query_limit,
                        service,
                    )?;
                } else {
                    // Semantic search explicitly requested but disabled in config
                    anyhow::bail!("Semantic search is disabled in configuration");
                }
            }
            // If no service (remote mode), results pass through
            // Remote backend handles semantic search on its end
        }

        if count_only {
            println!("{} bookmarks found", bookmarks.len());
            return Ok(vec![]);
        }

        Ok(bookmarks)
    }

    /// Apply hybrid ranking (semantic + lexical with RRF fusion) to filtered bookmarks.
    ///
    /// Hybrid search combines:
    /// 1. Semantic search: embedding similarity with threshold filtering
    /// 2. Lexical search: keyword matching against title/description/tags
    /// 3. RRF fusion: combines rankings, boosting items that appear in both
    ///
    /// The threshold is applied to semantic scores BEFORE fusion, preserving
    /// its meaning as "minimum semantic similarity". Lexical matches can
    /// rescue items that would otherwise be excluded due to low semantic scores.
    fn apply_semantic_ranking(
        &self,
        bookmarks: Vec<Bookmark>,
        query: &str,
        threshold: Option<f32>,
        limit: Option<usize>,
        service: &SemanticSearchService,
    ) -> Result<Vec<Bookmark>> {
        use crate::semantic::hybrid::rrf_fusion;
        use crate::semantic::lexical::score_lexical;

        if bookmarks.is_empty() {
            return Ok(bookmarks);
        }

        // Get IDs for candidate filtering
        let candidate_ids: Vec<u64> = bookmarks.iter().map(|b| b.id).collect();
        let limit = limit.unwrap_or(bookmarks.len());

        // 1. Semantic search with threshold (get ALL matches, not limited)
        let semantic_ids = service
            .search(query, Some(&candidate_ids), threshold, bookmarks.len())
            .context("Semantic search failed")?;

        // 2. Lexical search (no threshold, just keyword matching)
        let bookmark_data: Vec<(u64, &str, &str, &[String])> = bookmarks
            .iter()
            .map(|b| (b.id, b.title.as_str(), b.description.as_str(), b.tags.as_slice()))
            .collect();
        let lexical_results = score_lexical(query, &bookmark_data);
        let lexical_ids: Vec<u64> = lexical_results.iter().map(|r| r.id).collect();

        // 3. RRF fusion
        let hybrid_results = rrf_fusion(&semantic_ids, &lexical_ids);

        // Build lookup map and reorder bookmarks
        let id_to_bookmark: HashMap<u64, Bookmark> =
            bookmarks.into_iter().map(|b| (b.id, b)).collect();

        let ranked_bookmarks: Vec<Bookmark> = hybrid_results
            .into_iter()
            .take(limit)
            .filter_map(|result| id_to_bookmark.get(&result.id).cloned())
            .collect();

        Ok(ranked_bookmarks)
    }

    /// Create a new bookmark with validation and business rules
    pub fn create_bookmark(&self, create: BookmarkCreate, opts: AddOpts) -> Result<Bookmark> {
        // Validate bookmark creation
        self.validate_bookmark_creation(&create)?;

        // Check for duplicates
        self.check_duplicate_bookmark(&create.url)?;

        // Create the bookmark
        let bookmark = self
            .backend
            .create(create, opts)
            .context("Failed to create bookmark")?;

        // Index for semantic search (best-effort, don't fail create on embedding error)
        self.index_bookmark_for_semantic(&bookmark);

        Ok(bookmark)
    }

    /// Index a bookmark for semantic search.
    ///
    /// Computes embedding and adds to vector index. Logs warnings on failure
    /// but does not propagate errors to avoid failing bookmark operations.
    fn index_bookmark_for_semantic(&self, bookmark: &Bookmark) {
        let service = match &self.semantic_service {
            Some(s) if s.is_enabled() => s,
            _ => return, // No service or disabled
        };

        // Preprocess content - skip if empty
        let content = match preprocess_content(
            &bookmark.title,
            &bookmark.description,
            &bookmark.tags,
            &bookmark.url,
        ) {
            Some(c) => c,
            None => {
                log::debug!(
                    "Skipping embedding for bookmark {} - no content",
                    bookmark.id
                );
                return;
            }
        };

        let hash = content_hash(
            &bookmark.title,
            &bookmark.description,
            &bookmark.tags,
            &bookmark.url,
        );
        let bookmark_id = bookmark.id;

        // Generate embedding and add to index
        let result = service.with_index_mut(|index, model| {
            match model.embed(&content) {
                Ok(embedding) => {
                    if let Err(e) = index.insert(bookmark_id, hash, embedding) {
                        log::warn!("Failed to insert embedding for bookmark {}: {}", bookmark_id, e);
                    }
                }
                Err(e) => {
                    log::warn!("Failed to generate embedding for bookmark {}: {}", bookmark_id, e);
                }
            }
        });

        if let Err(e) = result {
            log::warn!("Failed to access semantic index for bookmark {}: {}", bookmark_id, e);
            return;
        }

        // Persist to storage
        if let Err(e) = service.save_index() {
            log::warn!("Failed to save semantic index after adding bookmark {}: {}", bookmark_id, e);
        }
    }

    /// Revalidate a bookmark's semantic index entry after update.
    ///
    /// Compares content hashes to detect changes. Re-embeds only if content
    /// (title/description) has changed. Logs warnings on failure but does not
    /// propagate errors to avoid failing bookmark updates.
    fn revalidate_bookmark_for_semantic(&self, bookmark: &Bookmark) {
        let service = match &self.semantic_service {
            Some(s) if s.is_enabled() => s,
            _ => return, // No service or disabled
        };

        // Compute current content hash
        let new_hash = content_hash(
            &bookmark.title,
            &bookmark.description,
            &bookmark.tags,
            &bookmark.url,
        );
        let bookmark_id = bookmark.id;

        // Check if hash changed (requires index access)
        let needs_reembed = service
            .with_index_mut(|index, _model| {
                match index.get(bookmark_id) {
                    Some(entry) => entry.content_hash != new_hash,
                    None => true, // Entry missing, needs embedding
                }
            })
            .unwrap_or(true); // On error, assume needs re-embed

        if !needs_reembed {
            log::debug!(
                "Bookmark {} content unchanged, skipping re-embed",
                bookmark_id
            );
            return;
        }

        // Preprocess content - skip if empty
        let content = match preprocess_content(
            &bookmark.title,
            &bookmark.description,
            &bookmark.tags,
            &bookmark.url,
        ) {
            Some(c) => c,
            None => {
                // Content is now empty - remove from index
                log::debug!(
                    "Bookmark {} has no content, removing from semantic index",
                    bookmark_id
                );
                let _ = service.with_index_mut(|index, _| {
                    index.remove(bookmark_id);
                });
                let _ = service.save_index();
                return;
            }
        };

        // Generate embedding and update index
        let result = service.with_index_mut(|index, model| {
            match model.embed(&content) {
                Ok(embedding) => {
                    if let Err(e) = index.insert(bookmark_id, new_hash, embedding) {
                        log::warn!(
                            "Failed to update embedding for bookmark {}: {}",
                            bookmark_id,
                            e
                        );
                    }
                }
                Err(e) => {
                    log::warn!(
                        "Failed to generate embedding for bookmark {}: {}",
                        bookmark_id,
                        e
                    );
                }
            }
        });

        if let Err(e) = result {
            log::warn!(
                "Failed to access semantic index for bookmark {}: {}",
                bookmark_id,
                e
            );
            return;
        }

        // Persist to storage
        if let Err(e) = service.save_index() {
            log::warn!(
                "Failed to save semantic index after updating bookmark {}: {}",
                bookmark_id,
                e
            );
        }
    }

    /// Ensure the semantic index is reconciled with current bookmark state.
    ///
    /// Fetches all bookmarks and reconciles the index on first call per session.
    /// Subsequent calls are no-ops. Failures are logged but do not block search.
    fn ensure_index_reconciled(&self, service: &SemanticSearchService) {
        // Skip if already reconciled this session
        if service.is_reconciled() {
            return;
        }

        // Fetch all bookmarks for reconciliation
        let all_bookmarks = match self.backend.search(SearchQuery::default()) {
            Ok(bookmarks) => bookmarks,
            Err(e) => {
                log::warn!("Failed to fetch bookmarks for reconciliation: {}", e);
                return;
            }
        };

        // Prepare bookmark data: (id, content_hash, preprocessed_content)
        // Only include bookmarks with embeddable content
        let bookmark_data: Vec<(u64, u64, String)> = all_bookmarks
            .iter()
            .filter_map(|b| {
                preprocess_content(&b.title, &b.description, &b.tags, &b.url).map(|content| {
                    let hash = content_hash(&b.title, &b.description, &b.tags, &b.url);
                    (b.id, hash, content)
                })
            })
            .collect();

        // Perform reconciliation
        match service.reconcile(&bookmark_data) {
            Ok(result) => {
                if result.has_changes() {
                    log::info!(
                        "Index reconciliation: {} orphans removed, {} stale re-embedded, {} missing embedded",
                        result.orphans_removed,
                        result.stale_reembedded,
                        result.missing_embedded
                    );
                }
            }
            Err(e) => {
                log::warn!("Index reconciliation failed: {}", e);
            }
        }
    }

    /// Update an existing bookmark
    pub fn update_bookmark(&self, id: u64, update: BookmarkUpdate) -> Result<Bookmark> {
        // Validate the update
        self.validate_bookmark_update(&update)?;

        // Check for conflicts if URL is being updated
        if let Some(ref new_url) = update.url {
            self.check_url_conflict(id, new_url)?;
        }

        // Check if embedded content fields are being updated for semantic revalidation
        // Embedded content includes: title, description, tags, and URL domain
        let content_changed = update.title.is_some()
            || update.description.is_some()
            || update.tags.is_some()
            || update.url.is_some();

        // Perform the update
        let bookmark = self
            .backend
            .update(id, update)
            .context("Failed to update bookmark")?;

        // Revalidate semantic index if content changed (best-effort)
        if content_changed {
            self.revalidate_bookmark_for_semantic(&bookmark);
        }

        Ok(bookmark)
    }

    /// Delete a bookmark by ID
    pub fn delete_bookmark(&self, id: u64) -> Result<()> {
        // Verify the bookmark exists before deletion
        let _existing = self
            .backend
            .search(SearchQuery {
                id: Some(id),
                ..Default::default()
            })
            .context("Failed to verify bookmark exists")?;

        if _existing.is_empty() {
            anyhow::bail!("Bookmark with ID {} not found", id);
        }

        // Perform the deletion
        self.backend
            .delete(id)
            .context("Failed to delete bookmark")?;

        Ok(())
    }

    /// Search and update multiple bookmarks
    pub fn search_and_update(&self, query: SearchQuery, update: BookmarkUpdate) -> Result<usize> {
        // Validate the update
        self.validate_bookmark_update(&update)?;

        // Check for URL conflicts if updating URLs
        if let Some(ref new_url) = update.url {
            self.check_bulk_url_conflicts(&query, new_url)?;
        }

        // Perform the bulk update
        let count = self
            .backend
            .search_update(query, update)
            .context("Failed to perform bulk update")?;

        Ok(count)
    }

    /// Search and delete multiple bookmarks
    pub fn search_and_delete(&self, query: SearchQuery) -> Result<usize> {
        // Get the count before deletion for confirmation
        let bookmarks = self
            .backend
            .search(query.clone())
            .context("Failed to search bookmarks for deletion")?;

        if bookmarks.is_empty() {
            return Ok(0);
        }

        // Perform the bulk deletion
        let count = self
            .backend
            .search_delete(query)
            .context("Failed to perform bulk deletion")?;

        Ok(count)
    }

    // MARK: - Metadata Operations

    /// Refresh metadata for a specific bookmark
    pub fn refresh_metadata(&self, id: u64, opts: RefreshMetadataOpts) -> Result<()> {
        // Verify the bookmark exists
        let bookmarks = self
            .backend
            .search(SearchQuery {
                id: Some(id),
                ..Default::default()
            })
            .context("Failed to verify bookmark exists")?;

        if bookmarks.is_empty() {
            anyhow::bail!("Bookmark with ID {} not found", id);
        }

        // Refresh the metadata
        self.backend
            .refresh_metadata(id, opts)
            .context("Failed to refresh metadata")?;

        Ok(())
    }

    // MARK: - Statistics and Information

    /// Get the total count of bookmarks
    pub fn get_total_count(&self) -> Result<usize> {
        let count = self
            .backend
            .total()
            .context("Failed to get total bookmark count")?;

        Ok(count)
    }

    /// Get all available tags
    pub fn get_tags(&self) -> Result<Vec<String>> {
        let tags = self.backend.tags().context("Failed to get tags")?;

        Ok(tags)
    }

    // MARK: - Configuration Management

    /// Get the current configuration
    pub fn get_config(&self) -> Result<Arc<RwLock<Config>>> {
        let config = self
            .backend
            .config()
            .context("Failed to get configuration")?;

        Ok(config)
    }

    /// Update the configuration
    pub fn update_config(&self, config: Config) -> Result<()> {
        // Validate the configuration before updating
        self.validate_config(&config)?;

        // Update the configuration
        self.backend
            .update_config(config)
            .context("Failed to update configuration")?;

        Ok(())
    }

    // MARK: - Private Validation Methods

    /// Validate bookmark creation data
    fn validate_bookmark_creation(&self, create: &BookmarkCreate) -> Result<()> {
        if create.url.trim().is_empty() {
            anyhow::bail!("Bookmark URL cannot be empty");
        }

        if let Some(ref title) = create.title {
            if title.len() > 500 {
                anyhow::bail!("Bookmark title cannot exceed 500 characters");
            }
        }

        if let Some(ref description) = create.description {
            if description.len() > 2000 {
                anyhow::bail!("Bookmark description cannot exceed 2000 characters");
            }
        }

        if let Some(ref tags) = create.tags {
            for tag in tags {
                if tag.len() > 50 {
                    anyhow::bail!("Individual tags cannot exceed 50 characters");
                }
                if tag.contains(' ') {
                    anyhow::bail!("Tags cannot contain spaces");
                }
            }
        }

        Ok(())
    }

    /// Validate bookmark update data
    fn validate_bookmark_update(&self, update: &BookmarkUpdate) -> Result<()> {
        if let Some(ref title) = update.title {
            if title.len() > 500 {
                anyhow::bail!("Bookmark title cannot exceed 500 characters");
            }
        }

        if let Some(ref description) = update.description {
            if description.len() > 2000 {
                anyhow::bail!("Bookmark description cannot exceed 2000 characters");
            }
        }

        if let Some(ref tags) = update.tags {
            for tag in tags {
                if tag.len() > 50 {
                    anyhow::bail!("Individual tags cannot exceed 50 characters");
                }
                if tag.contains(' ') {
                    anyhow::bail!("Tags cannot contain spaces");
                }
            }
        }

        Ok(())
    }

    /// Check for duplicate bookmarks
    fn check_duplicate_bookmark(&self, url: &str) -> Result<()> {
        let existing = self
            .backend
            .search(SearchQuery {
                url: Some(url.to_string()),
                exact: true,
                limit: Some(1),
                ..Default::default()
            })
            .context("Failed to check for duplicate bookmarks")?;
        log::info!("Existing bookmarks with URL '{}': {:?}", url, existing);

        if !existing.is_empty() {
            anyhow::bail!("Bookmark with URL '{}' already exists", url);
        }

        Ok(())
    }

    /// Check for URL conflicts when updating
    fn check_url_conflict(&self, exclude_id: u64, new_url: &str) -> Result<()> {
        let existing = self
            .backend
            .search(SearchQuery {
                url: Some(new_url.to_string()),
                exact: true,
                limit: Some(10),
                ..Default::default()
            })
            .context("Failed to check for URL conflicts")?;

        let conflicting = existing
            .iter()
            .filter(|b| b.id != exclude_id)
            .collect::<Vec<_>>();

        if !conflicting.is_empty() {
            anyhow::bail!("URL '{}' is already used by another bookmark", new_url);
        }

        Ok(())
    }

    /// Check for bulk URL conflicts
    fn check_bulk_url_conflicts(&self, query: &SearchQuery, new_url: &str) -> Result<()> {
        let bookmarks = self
            .backend
            .search(query.clone())
            .context("Failed to search bookmarks for bulk update")?;

        for bookmark in bookmarks {
            if bookmark.url != new_url {
                self.check_url_conflict(bookmark.id, new_url)?;
            }
        }

        Ok(())
    }

    /// Validate configuration
    fn validate_config(&self, config: &Config) -> Result<()> {
        if config.task_queue_max_threads == 0 {
            anyhow::bail!("Task queue max threads cannot be 0");
        }

        if config.task_queue_max_threads > 100 {
            anyhow::bail!("Task queue max threads cannot exceed 100");
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::backend::{AddOpts, AppBackend, RefreshMetadataOpts};
    use crate::app::errors::AppError;
    use crate::bookmarks::{Bookmark, BookmarkCreate, BookmarkUpdate};
    use crate::config::SemanticSearchConfig;
    use std::path::PathBuf;

    /// Mock backend that returns preconfigured bookmarks
    struct MockBackend {
        bookmarks: Vec<Bookmark>,
    }

    impl MockBackend {
        fn new(bookmarks: Vec<Bookmark>) -> Self {
            Self { bookmarks }
        }
    }

    impl AppBackend for MockBackend {
        fn create(&self, _: BookmarkCreate, _: AddOpts) -> anyhow::Result<Bookmark, AppError> {
            unimplemented!()
        }

        fn refresh_metadata(&self, _: u64, _: RefreshMetadataOpts) -> anyhow::Result<(), AppError> {
            unimplemented!()
        }

        fn update(&self, _: u64, _: BookmarkUpdate) -> anyhow::Result<Bookmark, AppError> {
            unimplemented!()
        }

        fn delete(&self, _: u64) -> anyhow::Result<(), AppError> {
            unimplemented!()
        }

        fn search_delete(&self, _: SearchQuery) -> anyhow::Result<usize, AppError> {
            unimplemented!()
        }

        fn search_update(&self, _: SearchQuery, _: BookmarkUpdate) -> anyhow::Result<usize, AppError> {
            unimplemented!()
        }

        fn total(&self) -> anyhow::Result<usize, AppError> {
            Ok(self.bookmarks.len())
        }

        fn tags(&self) -> anyhow::Result<Vec<String>, AppError> {
            unimplemented!()
        }

        fn search(&self, _query: SearchQuery) -> anyhow::Result<Vec<Bookmark>, AppError> {
            Ok(self.bookmarks.clone())
        }

        fn config(&self) -> anyhow::Result<Arc<RwLock<Config>>, AppError> {
            Ok(Arc::new(RwLock::new(Config::default())))
        }

        fn update_config(&self, _: Config) -> anyhow::Result<(), AppError> {
            unimplemented!()
        }
    }

    fn create_test_bookmark(id: u64, title: &str) -> Bookmark {
        Bookmark {
            id,
            title: title.to_string(),
            url: format!("https://example.com/{}", id),
            description: String::new(),
            tags: vec![],
            image_id: None,
            icon_id: None,
        }
    }

    fn disabled_semantic_config() -> SemanticSearchConfig {
        SemanticSearchConfig {
            enabled: false,
            model: "all-MiniLM-L6-v2".to_string(),
            default_threshold: 0.35,
            embedding_parallelism: "auto".to_string(),
            download_timeout_secs: 300,
        }
    }

    fn enabled_semantic_config() -> SemanticSearchConfig {
        SemanticSearchConfig {
            enabled: true,
            ..disabled_semantic_config()
        }
    }

    #[test]
    fn test_search_without_semantic_returns_all_results() {
        let bookmarks = vec![
            create_test_bookmark(1, "Machine Learning Guide"),
            create_test_bookmark(2, "Cooking Recipes"),
        ];
        let backend = Box::new(MockBackend::new(bookmarks.clone()));
        let service = AppService::new(backend);

        let query = SearchQuery::default();
        let results = service.search_bookmarks(query, false).unwrap();

        assert_eq!(results.len(), 2);
    }

    #[test]
    fn test_search_with_semantic_disabled_returns_error() {
        // GIVEN: A service with semantic search disabled
        let bookmarks = vec![
            create_test_bookmark(1, "Machine Learning Guide"),
            create_test_bookmark(2, "Cooking Recipes"),
        ];
        let backend = Box::new(MockBackend::new(bookmarks));

        let config = disabled_semantic_config();
        let semantic_service = Arc::new(SemanticSearchService::new(
            config,
            PathBuf::from("/tmp/test"),
        ));

        let service = AppService::with_semantic(backend, semantic_service);

        // WHEN: A search is performed with semantic parameter
        let query = SearchQuery {
            semantic: Some("artificial intelligence".to_string()),
            ..Default::default()
        };
        let result = service.search_bookmarks(query, false);

        // THEN: An error is returned indicating semantic search is disabled
        assert!(result.is_err());
        let err_msg = result.unwrap_err().to_string();
        assert!(
            err_msg.contains("disabled") || err_msg.contains("Disabled"),
            "Expected error message to mention 'disabled', got: {}",
            err_msg
        );
    }

    #[test]
    fn test_search_without_semantic_param_works_when_disabled() {
        // GIVEN: A service with semantic search disabled
        let bookmarks = vec![
            create_test_bookmark(1, "Machine Learning Guide"),
            create_test_bookmark(2, "Cooking Recipes"),
        ];
        let backend = Box::new(MockBackend::new(bookmarks.clone()));

        let config = disabled_semantic_config();
        let semantic_service = Arc::new(SemanticSearchService::new(
            config,
            PathBuf::from("/tmp/test"),
        ));

        let service = AppService::with_semantic(backend, semantic_service);

        // WHEN: A search is performed WITHOUT semantic parameter
        let query = SearchQuery::default();
        let result = service.search_bookmarks(query, false);

        // THEN: Results are returned normally (no error)
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 2);
    }

    #[test]
    fn test_search_with_semantic_no_service_passes_through() {
        // GIVEN: A service without semantic service (remote mode)
        let bookmarks = vec![
            create_test_bookmark(1, "Machine Learning Guide"),
        ];
        let backend = Box::new(MockBackend::new(bookmarks.clone()));
        let service = AppService::new(backend);

        // WHEN: A search is performed with semantic parameter
        let query = SearchQuery {
            semantic: Some("AI".to_string()),
            ..Default::default()
        };
        let result = service.search_bookmarks(query, false);

        // THEN: Results pass through (remote backend handles semantic)
        assert!(result.is_ok());
        assert_eq!(result.unwrap().len(), 1);
    }
}
