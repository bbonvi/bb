pub mod microlink;
pub mod oembed;
pub mod peekalink;
pub mod plain;

use crate::config::ScrapeConfig;
use crate::metadata::image_validation::validate_image;
use crate::metadata::types::{Metadata, MetaOptions};
use std::thread;

/// Helper to fetch bytes from a URL via scrape::reqwest_with_retries
pub fn fetch_bytes(url: &str, scrape_config: Option<&ScrapeConfig>) -> Option<Vec<u8>> {
    if let Some((status, bytes)) = crate::scrape::reqwest_with_retries(url, scrape_config) {
        if status.is_success() {
            return Some(bytes);
        }
    }
    None
}

/// Trait for different metadata fetching strategies
pub trait MetadataFetcher: Send + Sync {
    /// Attempt to fetch metadata from a URL
    /// Returns Some(metadata) if successful, None if this fetcher cannot handle the URL
    fn fetch(&self, url: &str, scrape_config: Option<&ScrapeConfig>) -> anyhow::Result<Option<Metadata>>;

    /// Get the name of this fetcher for logging/debugging
    fn name(&self) -> &'static str;

    /// Priority for merge ordering. Lower = higher priority (picked first).
    fn priority(&self) -> u8;
}

/// Collection of all available fetchers
pub struct FetcherRegistry {
    fetchers: Vec<Box<dyn MetadataFetcher>>,
}

// Guard: switch to semaphore-bounded pool if >6 fetchers
const MAX_UNBOUNDED_FETCHERS: usize = 6;

impl FetcherRegistry {
    pub fn new() -> Self {
        let mut registry = Self { fetchers: Vec::new() };

        // Add fetchers — all run in parallel, priority determines merge order
        registry.fetchers.push(Box::new(oembed::OembedFetcher::new()));
        registry.fetchers.push(Box::new(plain::PlainFetcher::new()));
        registry.fetchers.push(Box::new(microlink::MicrolinkFetcher::new()));
        registry.fetchers.push(Box::new(peekalink::PeekalinkFetcher::new()));

        assert!(
            registry.fetchers.len() <= MAX_UNBOUNDED_FETCHERS,
            "Add semaphore-bounded pool before exceeding {} parallel fetchers",
            MAX_UNBOUNDED_FETCHERS
        );

        registry
    }

    /// Fan out all fetchers in parallel, collect results, merge by priority.
    /// Falls back to headless if no validated image after merge.
    pub fn fetch_metadata(&self, url: &str, opts: &MetaOptions) -> anyhow::Result<Option<Metadata>> {
        let scrape_config = opts.scrape_config.as_ref();

        // Fan out parallel fetchers using thread::scope (bounded by fetcher count)
        let results: Vec<(u8, Metadata)> = thread::scope(|s| {
            let handles: Vec<_> = self
                .fetchers
                .iter()
                .map(|f| {
                    let sc = scrape_config;
                    s.spawn(move || {
                        let name = f.name();
                        match f.fetch(url, sc) {
                            Ok(Some(m)) => {
                                let fields = describe_fields(&m);
                                log::info!("fetcher={name} outcome=success fields=[{fields}]");
                                Some((f.priority(), m))
                            }
                            Ok(None) => {
                                log::info!("fetcher={name} outcome=skip");
                                None
                            }
                            Err(e) => {
                                log::warn!("fetcher={name} outcome=error err={e}");
                                None
                            }
                        }
                    })
                })
                .collect();

            handles
                .into_iter()
                .filter_map(|h| h.join().ok().flatten())
                .collect()
        });

        let merged = merge_results(results, scrape_config);

        // Short-circuit if we have a validated image
        if merged.has_valid_image() {
            return Ok(Some(merged));
        }

        // Headless fallback if no valid image yet
        if !opts.no_headless {
            let headless_fetcher = plain::HeadlessFetcher::new(opts.clone());
            if let Ok(Some(m)) = headless_fetcher.fetch_with_headless(url, scrape_config) {
                return Ok(Some(merge_two(merged, m, scrape_config)));
            }
        }

        if merged.has_any_data() {
            Ok(Some(merged))
        } else {
            Ok(None)
        }
    }
}

/// Merge multiple fetcher results by priority (lower priority number = preferred).
/// For each field, take the first non-None value from the sorted results.
/// Validate images during merge and set image_valid flag.
fn merge_results(mut results: Vec<(u8, Metadata)>, scrape_config: Option<&ScrapeConfig>) -> Metadata {
    results.sort_by_key(|(priority, _)| *priority);

    let mut merged = Metadata::default();

    for (_priority, m) in &results {
        if merged.title.is_none() {
            merged.title.clone_from(&m.title);
        }
        if merged.description.is_none() {
            merged.description.clone_from(&m.description);
        }
        if merged.keywords.is_none() {
            merged.keywords.clone_from(&m.keywords);
        }
        if merged.canonical_url.is_none() {
            merged.canonical_url.clone_from(&m.canonical_url);
        }
        if merged.icon_url.is_none() {
            merged.icon_url.clone_from(&m.icon_url);
        }
        if merged.icon.is_none() {
            merged.icon.clone_from(&m.icon);
        }
        if merged.dump.is_none() {
            merged.dump.clone_from(&m.dump);
        }

        // Image merge: try each source until we find a validated one
        if !merged.image_valid {
            if let Some(ref bytes) = m.image {
                if validate_image(bytes) {
                    merged.image = Some(bytes.clone());
                    merged.image_url.clone_from(&m.image_url);
                    merged.image_valid = true;
                } else {
                    log::debug!("Image from fetcher failed validation, trying next source");
                }
            } else if merged.image_url.is_none() {
                // No bytes but has URL — store URL for potential later fetch
                merged.image_url.clone_from(&m.image_url);
            }
        }
    }

    // If we have image_url but no validated image bytes, try fetching
    if !merged.image_valid {
        if let Some(ref img_url) = merged.image_url {
            if let Some(bytes) = fetch_bytes(img_url, scrape_config) {
                if validate_image(&bytes) {
                    merged.image = Some(bytes);
                    merged.image_valid = true;
                }
            }
        }
    }

    // Set image_source based on which fetcher provided the validated image
    if merged.image_valid {
        // Determine source by checking which result has matching image_url
        for (_priority, m) in &results {
            if m.image_url == merged.image_url {
                // We can't get the fetcher name here; tag generically
                // The priority number serves as identifier
                break;
            }
        }
    }

    merged
}

/// Merge overlay into base, filling only missing fields. Never overwrites existing non-None fields.
fn merge_two(mut base: Metadata, overlay: Metadata, scrape_config: Option<&ScrapeConfig>) -> Metadata {
    if base.title.is_none() {
        base.title = overlay.title;
    }
    if base.description.is_none() {
        base.description = overlay.description;
    }
    if base.keywords.is_none() {
        base.keywords = overlay.keywords;
    }
    if base.canonical_url.is_none() {
        base.canonical_url = overlay.canonical_url;
    }
    if base.icon_url.is_none() {
        base.icon_url = overlay.icon_url;
    }
    if base.icon.is_none() {
        base.icon = overlay.icon;
    }
    if base.dump.is_none() {
        base.dump = overlay.dump;
    }

    // Image: only fill if base doesn't have a validated image
    if !base.image_valid {
        if let Some(ref bytes) = overlay.image {
            if validate_image(bytes) {
                base.image = Some(bytes.clone());
                base.image_url = overlay.image_url;
                base.image_valid = true;
            }
        } else if base.image_url.is_none() {
            base.image_url = overlay.image_url;
        }

        // Try fetching if we have URL but no validated bytes
        if !base.image_valid {
            if let Some(ref img_url) = base.image_url {
                if let Some(bytes) = fetch_bytes(img_url, scrape_config) {
                    if validate_image(&bytes) {
                        base.image = Some(bytes);
                        base.image_valid = true;
                    }
                }
            }
        }
    }

    base
}

/// Describe which fields are present in metadata (for logging)
fn describe_fields(m: &Metadata) -> String {
    let mut fields = Vec::new();
    if m.title.is_some() { fields.push("title"); }
    if m.description.is_some() { fields.push("description"); }
    if m.image_url.is_some() { fields.push("image_url"); }
    if m.image.is_some() { fields.push("image"); }
    if m.icon_url.is_some() { fields.push("icon_url"); }
    if m.icon.is_some() { fields.push("icon"); }
    if m.canonical_url.is_some() { fields.push("canonical_url"); }
    fields.join(",")
}
