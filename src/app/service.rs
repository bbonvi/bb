use crate::{
    app::backend::{AddOpts, AppBackend, RefreshMetadataOpts},
    bookmarks::{BookmarkCreate, BookmarkUpdate, SearchQuery, Bookmark},
    config::Config,
};
use anyhow::{Result, Context};
use std::sync::{Arc, RwLock};

/// Application service layer that provides business logic and orchestrates operations
pub struct AppService {
    backend: Box<dyn AppBackend>,
}

impl AppService {
    /// Create a new application service with the given backend
    pub fn new(backend: Box<dyn AppBackend>) -> Self {
        Self { backend }
    }

    // MARK: - Bookmark Operations

    /// Search bookmarks with optional count-only mode
    pub fn search_bookmarks(
        &self,
        query: SearchQuery,
        count_only: bool,
    ) -> Result<Vec<Bookmark>> {
        let bookmarks = self.backend.search(query)
            .context("Failed to search bookmarks")?;
        
        if count_only {
            println!("{} bookmarks found", bookmarks.len());
            return Ok(vec![]);
        }
        
        Ok(bookmarks)
    }

    /// Create a new bookmark with validation and business rules
    pub fn create_bookmark(
        &self,
        create: BookmarkCreate,
        opts: AddOpts,
    ) -> Result<Bookmark> {
        // Validate bookmark creation
        self.validate_bookmark_creation(&create)?;
        
        // Check for duplicates
        self.check_duplicate_bookmark(&create.url)?;
        
        // Create the bookmark
        let bookmark = self.backend.create(create, opts)
            .context("Failed to create bookmark")?;
        
        Ok(bookmark)
    }

    /// Update an existing bookmark
    pub fn update_bookmark(
        &self,
        id: u64,
        update: BookmarkUpdate,
    ) -> Result<Bookmark> {
        // Validate the update
        self.validate_bookmark_update(&update)?;
        
        // Check for conflicts if URL is being updated
        if let Some(ref new_url) = update.url {
            self.check_url_conflict(id, new_url)?;
        }
        
        // Perform the update
        let bookmark = self.backend.update(id, update)
            .context("Failed to update bookmark")?;
        
        Ok(bookmark)
    }

    /// Delete a bookmark by ID
    pub fn delete_bookmark(&self, id: u64) -> Result<()> {
        // Verify the bookmark exists before deletion
        let _existing = self.backend.search(SearchQuery {
            id: Some(id),
            ..Default::default()
        }).context("Failed to verify bookmark exists")?;
        
        if _existing.is_empty() {
            anyhow::bail!("Bookmark with ID {} not found", id);
        }
        
        // Perform the deletion
        self.backend.delete(id)
            .context("Failed to delete bookmark")?;
        
        Ok(())
    }

    /// Search and update multiple bookmarks
    pub fn search_and_update(
        &self,
        query: SearchQuery,
        update: BookmarkUpdate,
    ) -> Result<usize> {
        // Validate the update
        self.validate_bookmark_update(&update)?;
        
        // Check for URL conflicts if updating URLs
        if let Some(ref new_url) = update.url {
            self.check_bulk_url_conflicts(&query, new_url)?;
        }
        
        // Perform the bulk update
        let count = self.backend.search_update(query, update)
            .context("Failed to perform bulk update")?;
        
        Ok(count)
    }

    /// Search and delete multiple bookmarks
    pub fn search_and_delete(&self, query: SearchQuery) -> Result<usize> {
        // Get the count before deletion for confirmation
        let bookmarks = self.backend.search(query.clone())
            .context("Failed to search bookmarks for deletion")?;
        
        if bookmarks.is_empty() {
            return Ok(0);
        }
        
        // Perform the bulk deletion
        let count = self.backend.search_delete(query)
            .context("Failed to perform bulk deletion")?;
        
        Ok(count)
    }

    // MARK: - Metadata Operations

    /// Refresh metadata for a specific bookmark
    pub fn refresh_metadata(
        &self,
        id: u64,
        opts: RefreshMetadataOpts,
    ) -> Result<()> {
        // Verify the bookmark exists
        let bookmarks = self.backend.search(SearchQuery {
            id: Some(id),
            ..Default::default()
        }).context("Failed to verify bookmark exists")?;
        
        if bookmarks.is_empty() {
            anyhow::bail!("Bookmark with ID {} not found", id);
        }
        
        // Refresh the metadata
        self.backend.refresh_metadata(id, opts)
            .context("Failed to refresh metadata")?;
        
        Ok(())
    }

    // MARK: - Statistics and Information

    /// Get the total count of bookmarks
    pub fn get_total_count(&self) -> Result<usize> {
        let count = self.backend.total()
            .context("Failed to get total bookmark count")?;
        
        Ok(count)
    }

    /// Get all available tags
    pub fn get_tags(&self) -> Result<Vec<String>> {
        let tags = self.backend.tags()
            .context("Failed to get tags")?;
        
        Ok(tags)
    }

    // MARK: - Configuration Management

    /// Get the current configuration
    pub fn get_config(&self) -> Result<Arc<RwLock<Config>>> {
        let config = self.backend.config()
            .context("Failed to get configuration")?;
        
        Ok(config)
    }

    /// Update the configuration
    pub fn update_config(&self, config: Config) -> Result<()> {
        // Validate the configuration before updating
        self.validate_config(&config)?;
        
        // Update the configuration
        self.backend.update_config(config)
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
        let existing = self.backend.search(SearchQuery {
            url: Some(url.to_string()),
            exact: true,
            limit: Some(1),
            ..Default::default()
        }).context("Failed to check for duplicate bookmarks")?;
        
        if !existing.is_empty() {
            anyhow::bail!("Bookmark with URL '{}' already exists", url);
        }
        
        Ok(())
    }

    /// Check for URL conflicts when updating
    fn check_url_conflict(&self, exclude_id: u64, new_url: &str) -> Result<()> {
        let existing = self.backend.search(SearchQuery {
            url: Some(new_url.to_string()),
            exact: true,
            limit: Some(10),
            ..Default::default()
        }).context("Failed to check for URL conflicts")?;
        
        let conflicting = existing.iter()
            .filter(|b| b.id != exclude_id)
            .collect::<Vec<_>>();
        
        if !conflicting.is_empty() {
            anyhow::bail!("URL '{}' is already used by another bookmark", new_url);
        }
        
        Ok(())
    }

    /// Check for bulk URL conflicts
    fn check_bulk_url_conflicts(&self, query: &SearchQuery, new_url: &str) -> Result<()> {
        let bookmarks = self.backend.search(query.clone())
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
