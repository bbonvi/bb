use crate::{
    app::service::AppService,
    bookmarks::{BookmarkCreate, BookmarkUpdate, SearchQuery},
    images,
    metadata::MetaOptions,
    parse_tags,
    storage::StorageManager,
    cli::{errors::CliResult, validation::{
        validate_search_query, validate_semantic_params, validate_bookmark_create,
        validate_tags, validate_url, validate_rule_input,
    }},
};
use indicatif::{ProgressBar, ProgressStyle};
use rayon::prelude::*;
use std::collections::HashMap;
use tracing::{debug, info, warn};

/// Command for searching bookmarks
#[derive(Debug, Clone)]
pub struct SearchCommand {
    pub query: SearchQuery,
    pub count_only: bool,
    pub action: Option<ActionCommand>,
}

/// Parameters for creating a search command
#[derive(Debug, Clone)]
pub struct SearchCommandParams {
    pub url: Option<String>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub tags: Option<String>,
    pub keyword: Option<String>,
    pub id: Option<u64>,
    pub exact: bool,
    pub semantic: Option<String>,
    pub threshold: Option<f32>,
    pub count: bool,
    pub action: Option<ActionCommand>,
}

impl SearchCommand {
    pub fn new(params: SearchCommandParams) -> CliResult<Self> {
        // Validate search query input
        validate_search_query(&params.url, &params.title, &params.description, &params.tags)?;

        // Validate semantic search parameters
        validate_semantic_params(&params.semantic, &params.threshold)?;

        let query = SearchQuery {
            id: params.id,
            title: params.title,
            url: params.url,
            description: params.description,
            tags: params.tags.map(parse_tags),
            keyword: params.keyword,
            exact: params.exact,
            semantic: params.semantic,
            threshold: params.threshold,
            limit: None,
            ..Default::default()
        };

        Ok(Self {
            query,
            count_only: params.count,
            action: params.action,
        })
    }

    pub fn execute(self, app_service: AppService) -> CliResult<()> {
        let bmarks = app_service.search_bookmarks(self.query.clone(), self.count_only)
            .map_err(|e| crate::cli::errors::CliError::database(e.to_string()))?;

        if bmarks.is_empty() {
            if self.action.is_some() {
                println!("0 items updated");
                return Ok(());
            }
            println!("{}", serde_json::to_string_pretty(&bmarks)
                .map_err(|e| crate::cli::errors::CliError::invalid_input(e.to_string()))?);
            return Ok(());
        }

        if self.count_only {
            println!("{} bookmarks found", bmarks.len());
            return Ok(());
        }

        if let Some(action) = self.action {
            action.execute(bmarks, self.query, app_service)
        } else {
            println!("{}", serde_json::to_string_pretty(&bmarks)
                .map_err(|e| crate::cli::errors::CliError::invalid_input(e.to_string()))?);
            Ok(())
        }
    }
}

/// Command for adding bookmarks
#[derive(Debug, Clone)]
pub struct AddCommand {
    pub url: Option<String>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub tags: Option<String>,
    pub options: AddOptions,
}

#[derive(Debug, Clone)]
pub struct AddOptions {
    pub use_editor: bool,
    pub no_https_upgrade: bool,
    pub no_headless: bool,
    pub no_meta: bool,
    pub async_meta: bool,
}

impl AddCommand {
    pub fn new(
        url: Option<String>,
        title: Option<String>,
        description: Option<String>,
        tags: Option<String>,
        options: AddOptions,
    ) -> CliResult<Self> {
        // Validate bookmark creation input
        validate_bookmark_create(&url, &title, &description, &tags)?;

        Ok(Self {
            url,
            title,
            description,
            tags,
            options,
        })
    }

    pub fn execute(self, app_service: AppService) -> CliResult<()> {
        let mut url = self.url;
        let mut title = self.title;
        let mut description = self.description;
        let mut tags = self.tags;

        if self.options.use_editor {
            let mut current_tags = app_service.get_tags()
                .map_err(|e| crate::cli::errors::CliError::database(e.to_string()))?;
            current_tags.sort();

            let mut editor_defaults = crate::editor::EditorDefaults {
                url: url.clone(),
                title: title.clone(),
                description: description.clone(),
                tags: tags.clone(),
                current_tags,
            };

            let config = app_service.get_config()
                .map_err(|e| crate::cli::errors::CliError::configuration(e.to_string()))?;
            let rules = &config.read().unwrap().rules;

            if let Some(u) = url {
                for rule in rules.iter() {
                    let record = crate::rules::Record {
                        url: u.clone(),
                        title: title.clone(),
                        description: description.clone(),
                        tags: tags.clone().map(parse_tags),
                    };

                    if !rule.is_match(&record) {
                        continue;
                    }

                    match &rule.action {
                        crate::rules::Action::UpdateBookmark {
                            title: rule_title,
                            description: rule_description,
                            tags: rule_tags,
                        } => {
                            if let Some(title) = rule_title {
                                editor_defaults.title = Some(title.clone());
                            }
                            if let Some(description) = rule_description {
                                editor_defaults.description = Some(description.clone());
                            }
                            if let Some(tags) = rule_tags {
                                let mut curr_tags = editor_defaults.tags
                                    .map(parse_tags)
                                    .unwrap_or_default();
                                curr_tags.append(&mut tags.clone());
                                editor_defaults.tags = Some(curr_tags.join(" "));
                            }
                        }
                    }
                }
            }

            let editor_bmark = crate::editor::edit(editor_defaults)
                .map_err(|e| crate::cli::errors::CliError::invalid_input(e.to_string()))?;

            url = Some(editor_bmark.url);
            if let crate::editor::EditorValue::Set(value) = editor_bmark.title {
                title = Some(value)
            }
            if let crate::editor::EditorValue::Set(value) = editor_bmark.description {
                description = Some(value)
            }
            if let crate::editor::EditorValue::Set(value) = editor_bmark.tags {
                tags = Some(value)
            }
        } else if url.is_none() {
            return Err(crate::cli::errors::CliError::validation("url", "URL cannot be empty"));
        }

        let url = url.unwrap_or_default();

        let bmark_create = BookmarkCreate {
            title,
            description,
            tags: tags.map(parse_tags),
            url,
            ..Default::default()
        };

        let add_opts = crate::app::backend::AddOpts {
            no_https_upgrade: self.options.no_https_upgrade,
            async_meta: self.options.async_meta,
            meta_opts: if self.options.no_meta {
                None
            } else {
                Some(MetaOptions { 
                    no_headless: self.options.no_headless 
                })
            },
            skip_rules: false,
        };

        let bmark = app_service.create_bookmark(bmark_create, add_opts)
            .map_err(|e| crate::cli::errors::CliError::database(e.to_string()))?;
        
        println!("{}", serde_json::to_string_pretty(&bmark)
            .map_err(|e| crate::cli::errors::CliError::invalid_input(e.to_string()))?);
        Ok(())
    }
}

/// Command for metadata operations
#[derive(Debug, Clone)]
pub struct MetaCommand {
    pub url: String,
    pub options: MetaOptions,
}

impl MetaCommand {
    pub fn new(url: String, options: MetaOptions) -> CliResult<Self> {
        // Validate URL
        validate_url(&url)?;
        
        Ok(Self { url, options })
    }

    pub fn execute(self) -> CliResult<()> {
        let meta = crate::metadata::fetch_meta(&self.url, self.options)
            .map_err(|e| crate::cli::errors::CliError::metadata(e.to_string()))?;

        if let Some(ref image) = meta.image {
            std::fs::write("screenshot.png", image)
                .map_err(|e| crate::cli::errors::CliError::storage(e.to_string()))?;
        }

        if let Some(ref icon) = meta.icon {
            std::fs::write("icon.png", icon)
                .map_err(|e| crate::cli::errors::CliError::storage(e.to_string()))?;
        }

        println!("{}", serde_json::to_string_pretty(&meta)
            .map_err(|e| crate::cli::errors::CliError::invalid_input(e.to_string()))?);
        Ok(())
    }
}

/// Command for rule operations
#[derive(Debug, Clone)]
pub struct RuleCommand {
    pub action: RuleAction,
}

#[derive(Debug, Clone)]
pub enum RuleAction {
    Add {
        url: Option<String>,
        title: Option<String>,
        description: Option<String>,
        tags: Option<String>,
        update_action: RuleUpdateAction,
    },
    Delete,
    List,
}

#[derive(Debug, Clone)]
pub struct RuleUpdateAction {
    pub title: Option<String>,
    pub description: Option<String>,
    pub tags: Option<String>,
}

impl RuleCommand {
    pub fn new(action: RuleAction) -> CliResult<Self> {
        // Validate rule input if it's an Add action
        if let RuleAction::Add { url, title, description, tags, .. } = &action {
            validate_rule_input(url, title, description, tags)?;
        }
        
        Ok(Self { action })
    }

    pub fn execute(self, config: &mut crate::config::Config) -> CliResult<()> {
        match self.action {
            RuleAction::Add {
                url,
                title,
                description,
                tags,
                update_action,
            } => {
                let rule = crate::rules::Rule {
                    url: url.map(|u| u.to_lowercase()),
                    description: description.map(|d| d.to_lowercase()),
                    title: title.map(|d| d.to_lowercase()),
                    tags: tags.clone().map(parse_tags),
                    action: crate::rules::Action::UpdateBookmark {
                        title: update_action.title.map(|u| u.to_lowercase()),
                        description: update_action.description.map(|d| d.to_lowercase()),
                        tags: update_action.tags.clone().map(parse_tags),
                    },
                    comment: None,
                };
                config.rules.insert(0, rule);
                config.save()
                    .map_err(|e| crate::cli::errors::CliError::storage(e.to_string()))?;
            }
            RuleAction::Delete => {
                return Err(crate::cli::errors::CliError::not_supported("Delete rule"));
            }
            RuleAction::List => {
                for (idx, rule) in config.rules.iter().enumerate() {
                    if let Some(comment) = &rule.comment {
                        println!("Rule #{} // {comment}", idx + 1);
                    } else {
                        println!("Rule #{}", idx + 1);
                    }
                    if let Some(url) = &rule.url {
                        println!("  url: {url:#?}");
                    }
                    if let Some(title) = &rule.title {
                        println!("  title: {title:#?}");
                    }
                    if let Some(description) = &rule.description {
                        println!("  description: {description:#?}");
                    }
                    if let Some(tags) = &rule.tags {
                        println!("  tags: {tags:?}");
                    }

                    match &rule.action {
                        crate::rules::Action::UpdateBookmark {
                            title,
                            description,
                            tags,
                        } => {
                            println!("  UpdateBookmark:");
                            if let Some(title) = &title {
                                println!("    title: {title}");
                            }
                            if let Some(description) = &description {
                                println!("    description: {description}");
                            }
                            if let Some(tags) = &tags {
                                println!("    tags: {tags:?}");
                            }
                        }
                    }
                    println!();
                }
            }
        }
        Ok(())
    }
}

/// Command for actions on search results
#[derive(Debug, Clone)]
pub enum ActionCommand {
    Update {
        url: Option<String>,
        title: Option<String>,
        description: Option<String>,
        tags: Option<String>,
        append_tags: Option<String>,
        remove_tags: Option<String>,
    },
    Delete {
        yes: bool,
        force: bool,
    },
}

impl ActionCommand {
    pub fn execute(
        self,
        bmarks: Vec<crate::bookmarks::Bookmark>,
        query: SearchQuery,
        app_service: AppService,
    ) -> CliResult<()> {
        match self {
            ActionCommand::Update {
                url,
                title,
                description,
                tags,
                append_tags,
                remove_tags,
            } => {
                // Validate update input
                validate_bookmark_create(&url, &title, &description, &tags)?;
                if let Some(ref append_tags) = append_tags {
                    validate_tags(append_tags)?;
                }
                if let Some(ref remove_tags) = remove_tags {
                    validate_tags(remove_tags)?;
                }

                let bmark_update = BookmarkUpdate {
                    title,
                    description,
                    tags: tags.map(parse_tags),
                    url,
                    append_tags: append_tags.map(parse_tags),
                    remove_tags: remove_tags.map(parse_tags),
                    ..Default::default()
                };

                let is_entire_db = query.url.is_none()
                    && query.title.is_none()
                    && query.description.is_none()
                    && query.tags.is_none()
                    && query.id.is_none();

                if bmark_update.title.is_none()
                    && bmark_update.description.is_none()
                    && bmark_update.tags.is_none()
                    && bmark_update.url.is_none()
                    && bmark_update.remove_tags.is_none()
                    && bmark_update.append_tags.is_none()
                {
                    println!("The update did nothing");
                    return Ok(());
                }

                if is_entire_db {
                    match inquire::prompt_confirmation(
                        format!("You are about to update every single bookmark ({} items). Are you really sure?", bmarks.len()),
                    ) {
                        inquire::error::InquireResult::Ok(true) => {}
                        inquire::error::InquireResult::Ok(false) => return Ok(()),
                        inquire::error::InquireResult::Err(err) => return Err(crate::cli::errors::CliError::invalid_input(err.to_string())),
                    }
                }

                let count = app_service.search_and_update(query, bmark_update)
                    .map_err(|e| crate::cli::errors::CliError::database(e.to_string()))?;
                println!("{} items updated", count);
                Ok(())
            }
            ActionCommand::Delete { yes, force } => {
                let is_wipe = !force
                    && query.url.is_none()
                    && query.title.is_none()
                    && query.description.is_none()
                    && query.tags.is_none()
                    && query.id.is_none();

                if !yes {
                    match inquire::prompt_confirmation(format!(
                        "Are you sure you want to delete {} bookmarks?",
                        bmarks.len()
                    )) {
                        inquire::error::InquireResult::Ok(true) => {}
                        inquire::error::InquireResult::Ok(false) => return Ok(()),
                        inquire::error::InquireResult::Err(err) => return Err(crate::cli::errors::CliError::invalid_input(err.to_string())),
                    }
                }

                if is_wipe {
                    match inquire::prompt_confirmation(
                        "You are about to wipe your entire database. Are you really sure?",
                    ) {
                        inquire::error::InquireResult::Ok(true) => {}
                        inquire::error::InquireResult::Ok(false) => return Ok(()),
                        inquire::error::InquireResult::Err(err) => return Err(crate::cli::errors::CliError::invalid_input(err.to_string())),
                    }
                }

                let count = app_service.search_and_delete(query)
                    .map_err(|e| crate::cli::errors::CliError::database(e.to_string()))?;
                println!("{} items removed", count);
                Ok(())
            }
        }
    }
}

/// Statistics for image compression preview
#[derive(Debug, Default)]
pub struct CompressionStats {
    pub total_images: usize,
    pub already_optimal: usize,
    pub to_compress: usize,
    pub failed_to_read: usize,
}

/// Image pending compression
#[derive(Debug)]
struct ImageToCompress {
    image_id: String,
    bookmark_ids: Vec<u64>,
    data: Vec<u8>,
}

/// Result of parallel compression phase
struct CompressedImage {
    image_id: String,
    bookmark_ids: Vec<u64>,
    result: anyhow::Result<images::CompressionResult>,
}

/// Command for batch image compression
#[derive(Debug)]
pub struct CompressCommand {
    pub dry_run: bool,
    pub skip_confirm: bool,
}

impl CompressCommand {
    pub fn new(dry_run: bool, skip_confirm: bool) -> Self {
        Self { dry_run, skip_confirm }
    }

    pub fn execute<S: StorageManager>(
        self,
        storage: &S,
        bookmarks: &[crate::bookmarks::Bookmark],
        max_size: u32,
        quality: u8,
        update_bookmark: impl Fn(u64, crate::bookmarks::BookmarkUpdate) -> anyhow::Result<crate::bookmarks::Bookmark>,
    ) -> CliResult<()> {
        // Build map of image_id -> bookmark IDs that reference it
        let mut image_to_bookmarks: HashMap<String, Vec<u64>> = HashMap::new();
        for bmark in bookmarks {
            if let Some(ref image_id) = bmark.image_id {
                image_to_bookmarks
                    .entry(image_id.clone())
                    .or_default()
                    .push(bmark.id);
            }
        }

        if image_to_bookmarks.is_empty() {
            println!("No images found in bookmarks.");
            return Ok(());
        }

        // Analyze images with progress bar
        info!(count = image_to_bookmarks.len(), "scanning images");
        let mut stats = CompressionStats::default();
        let mut to_compress: Vec<ImageToCompress> = Vec::new();

        let scan_pb = ProgressBar::new(image_to_bookmarks.len() as u64);
        scan_pb.set_style(
            ProgressStyle::default_bar()
                .template("{spinner:.green} Scanning [{bar:40.cyan/blue}] {pos}/{len}")
                .unwrap()
                .progress_chars("#>-"),
        );

        for (image_id, bookmark_ids) in &image_to_bookmarks {
            stats.total_images += 1;
            scan_pb.inc(1);

            // Try to read the image
            if !storage.exists(image_id) {
                debug!(image_id, "image file not found");
                stats.failed_to_read += 1;
                continue;
            }

            let data = match storage.read(image_id) {
                Ok(d) => d,
                Err(e) => {
                    warn!(image_id, error = %e, "failed to read image");
                    stats.failed_to_read += 1;
                    continue;
                }
            };
            if data.is_empty() {
                warn!(image_id, "image file empty");
                stats.failed_to_read += 1;
                continue;
            }

            // Check if needs processing (fast - no compression)
            if !images::should_process(&data, max_size) {
                debug!(image_id, "already optimal");
                stats.already_optimal += 1;
                continue;
            }

            debug!(image_id, size = data.len(), "needs compression");
            stats.to_compress += 1;
            to_compress.push(ImageToCompress {
                image_id: image_id.clone(),
                bookmark_ids: bookmark_ids.clone(),
                data,
            });
        }

        scan_pb.finish_and_clear();

        // Display preview
        self.print_preview(&stats);

        if stats.to_compress == 0 {
            println!("\nNo images need compression.");
            return Ok(());
        }

        if self.dry_run {
            println!("\nDry run - no changes made.");
            return Ok(());
        }

        // Confirm
        if !self.skip_confirm {
            match inquire::prompt_confirmation("Proceed with compression?") {
                Ok(true) => {}
                Ok(false) => {
                    println!("Aborted.");
                    return Ok(());
                }
                Err(e) => return Err(crate::cli::errors::CliError::invalid_input(e.to_string())),
            }
        }

        // Phase 1: Compress images in parallel with progress bar
        info!(count = to_compress.len(), max_size, quality, "compressing images");
        let pb = ProgressBar::new(to_compress.len() as u64);
        pb.set_style(
            ProgressStyle::default_bar()
                .template("{spinner:.green} Compressing [{bar:40.cyan/blue}] {pos}/{len} ({eta})")
                .unwrap()
                .progress_chars("#>-"),
        );

        let compressed: Vec<CompressedImage> = to_compress
            .into_par_iter()
            .map(|img| {
                let result = images::compress_image(&img.data, max_size, quality);
                pb.inc(1);
                CompressedImage {
                    image_id: img.image_id,
                    bookmark_ids: img.bookmark_ids,
                    result,
                }
            })
            .collect();

        pb.finish_and_clear();

        // Phase 2: Apply results sequentially (I/O + bookmark updates)
        info!("writing compressed images to storage");
        let mut success_count = 0;
        let mut error_count = 0;

        for img in compressed {
            match img.result {
                Ok(result) => {
                    let compressed_size = result.data.len();
                    match self.apply_compression(&img.image_id, &img.bookmark_ids, result, storage, &update_bookmark) {
                        Ok(()) => {
                            debug!(image_id = img.image_id, compressed_size, "saved");
                            success_count += 1;
                        }
                        Err(e) => {
                            warn!(image_id = img.image_id, error = %e, "failed to save");
                            error_count += 1;
                        }
                    }
                }
                Err(e) => {
                    warn!(image_id = img.image_id, error = %e, "compression failed");
                    error_count += 1;
                }
            }
        }

        info!(success_count, error_count, "compression complete");
        println!("\nCompression complete:");
        println!("  Successful: {}", success_count);
        if error_count > 0 {
            println!("  Failed: {}", error_count);
        }

        Ok(())
    }

    /// Apply a pre-computed compression result: write file, update bookmarks, cleanup
    fn apply_compression<S: StorageManager>(
        &self,
        image_id: &str,
        bookmark_ids: &[u64],
        result: images::CompressionResult,
        storage: &S,
        update_bookmark: &impl Fn(u64, crate::bookmarks::BookmarkUpdate) -> anyhow::Result<crate::bookmarks::Bookmark>,
    ) -> anyhow::Result<()> {
        // Generate new filename with .webp extension
        let new_id = if image_id.ends_with(".webp") {
            image_id.to_string()
        } else {
            let base = image_id.rsplit_once('.').map(|(b, _)| b).unwrap_or(image_id);
            format!("{}.webp", base)
        };

        // Write new file
        storage.write(&new_id, &result.data)?;

        // Update bookmark references
        for &bookmark_id in bookmark_ids {
            let update = crate::bookmarks::BookmarkUpdate {
                image_id: Some(new_id.clone()),
                ..Default::default()
            };
            update_bookmark(bookmark_id, update)?;
        }

        // Delete old file if different
        if new_id != image_id {
            let _ = storage.delete(image_id);
        }

        Ok(())
    }

    fn print_preview(&self, stats: &CompressionStats) {
        println!("Image Compression Preview");
        println!("=========================");
        println!("Total images:     {}", stats.total_images);
        println!("Already optimal:  {} (skipped)", stats.already_optimal);
        println!("To compress:      {}", stats.to_compress);
        if stats.failed_to_read > 0 {
            println!("Failed to read:   {}", stats.failed_to_read);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // =========================================================================
    // SearchCommand construction tests (E.4)
    //
    // Verify that semantic/threshold flags are correctly wired through
    // SearchCommand to SearchQuery.
    // =========================================================================

    #[test]
    fn test_search_command_wires_semantic_to_query() {
        let params = SearchCommandParams {
            url: None,
            title: None,
            description: None,
            tags: None,
            keyword: None,
            id: None,
            exact: false,
            semantic: Some("machine learning AI".to_string()),
            threshold: None,
            count: false,
            action: None,
        };

        let cmd = SearchCommand::new(params).expect("Should create command");

        assert_eq!(
            cmd.query.semantic,
            Some("machine learning AI".to_string()),
            "Semantic query should be preserved in SearchQuery"
        );
    }

    #[test]
    fn test_search_command_wires_threshold_to_query() {
        let params = SearchCommandParams {
            url: None,
            title: None,
            description: None,
            tags: None,
            keyword: None,
            id: None,
            exact: false,
            semantic: Some("test query".to_string()),
            threshold: Some(0.7),
            count: false,
            action: None,
        };

        let cmd = SearchCommand::new(params).expect("Should create command");

        assert_eq!(
            cmd.query.threshold,
            Some(0.7),
            "Threshold should be preserved in SearchQuery"
        );
    }

    #[test]
    fn test_search_command_combined_filters_with_semantic() {
        let params = SearchCommandParams {
            url: None,
            title: Some("Rust".to_string()),
            description: None,
            tags: Some("programming".to_string()),
            keyword: None,
            id: None,
            exact: false,
            semantic: Some("systems programming".to_string()),
            threshold: Some(0.5),
            count: false,
            action: None,
        };

        let cmd = SearchCommand::new(params).expect("Should create command");

        // All fields should be wired through
        assert_eq!(cmd.query.title, Some("Rust".to_string()));
        assert_eq!(cmd.query.tags, Some(vec!["programming".to_string()]));
        assert_eq!(cmd.query.semantic, Some("systems programming".to_string()));
        assert_eq!(cmd.query.threshold, Some(0.5));
    }

    #[test]
    fn test_search_command_rejects_invalid_threshold() {
        let params = SearchCommandParams {
            url: None,
            title: None,
            description: None,
            tags: None,
            keyword: None,
            id: None,
            exact: false,
            semantic: Some("query".to_string()),
            threshold: Some(1.5), // Invalid: > 1.0
            count: false,
            action: None,
        };

        let result = SearchCommand::new(params);
        assert!(result.is_err(), "Should reject threshold > 1.0");
    }

    #[test]
    fn test_search_command_rejects_negative_threshold() {
        let params = SearchCommandParams {
            url: None,
            title: None,
            description: None,
            tags: None,
            keyword: None,
            id: None,
            exact: false,
            semantic: Some("query".to_string()),
            threshold: Some(-0.5), // Invalid: < 0.0
            count: false,
            action: None,
        };

        let result = SearchCommand::new(params);
        assert!(result.is_err(), "Should reject threshold < 0.0");
    }

    #[test]
    fn test_search_command_no_semantic_fields() {
        let params = SearchCommandParams {
            url: None,
            title: Some("test".to_string()),
            description: None,
            tags: None,
            keyword: None,
            id: None,
            exact: false,
            semantic: None,
            threshold: None,
            count: false,
            action: None,
        };

        let cmd = SearchCommand::new(params).expect("Should create command");

        assert!(cmd.query.semantic.is_none(), "No semantic should be set");
        assert!(cmd.query.threshold.is_none(), "No threshold should be set");
    }
}
