pub mod ddg;
pub mod iframely;
pub mod microlink;
pub mod oembed;
pub mod peekalink;
pub mod plain;
pub mod wayback;

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
}

/// Collection of all available fetchers
pub struct FetcherRegistry {
    fetchers: Vec<Box<dyn MetadataFetcher>>,
}

// Guard: switch to semaphore-bounded pool if >6 fetchers
const MAX_UNBOUNDED_FETCHERS: usize = 8;

impl FetcherRegistry {
    pub fn new(opts: &MetaOptions) -> Self {
        let mut registry = Self { fetchers: Vec::new() };

        // Build fetcher order from config, falling back to defaults
        let order = opts.scrape_config.as_ref()
            .map(|c| &c.fetcher_order)
            .filter(|o| !o.is_empty())
            .cloned()
            .unwrap_or_else(crate::config::default_fetcher_order);

        for name in &order {
            match name.as_str() {
                "oEmbed" => registry.fetchers.push(Box::new(oembed::OembedFetcher::new())),
                "Wayback" => registry.fetchers.push(Box::new(wayback::WaybackFetcher::new())),
                "Plain" => registry.fetchers.push(Box::new(plain::PlainFetcher::new())),
                "Microlink" => registry.fetchers.push(Box::new(microlink::MicrolinkFetcher::new())),
                "Peekalink" => registry.fetchers.push(Box::new(peekalink::PeekalinkFetcher::new())),
                "Iframely" => registry.fetchers.push(Box::new(iframely::IframelyFetcher::new())),
                "DDG" => registry.fetchers.push(Box::new(ddg::DdgFetcher::new())),
                other => log::warn!("Unknown fetcher in config: {other}"),
            }
        }

        // Headless is conditional, not part of the configurable order
        let always_headless = opts.scrape_config.as_ref()
            .map(|c| c.always_headless)
            .unwrap_or(false);
        if always_headless && !opts.no_headless {
            registry.fetchers.push(Box::new(plain::HeadlessParallelFetcher::new(opts.clone())));
        }

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
        let results: Vec<(u8, &str, Metadata)> = thread::scope(|s| {
            let handles: Vec<_> = self
                .fetchers
                .iter()
                .enumerate()
                .map(|(idx, f)| {
                    let sc = scrape_config;
                    s.spawn(move || {
                        let name = f.name();
                        match f.fetch(url, sc) {
                            Ok(Some(m)) => {
                                let fields = describe_fields(&m);
                                log::info!("fetcher={name} outcome=success fields=[{fields}]");
                                Some((idx as u8, name, m))
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

        // Short-circuit only if we have ALL key fields: validated image,
        // non-generic title, and a description. Missing or generic text
        // fields warrant a headless fallback attempt.
        // Skip fallback if headless already ran in parallel (always_headless).
        let already_ran_headless = opts.scrape_config.as_ref()
            .map(|c| c.always_headless)
            .unwrap_or(false);
        let needs_headless = !already_ran_headless
            && (!merged.has_valid_image()
                || merged.description.is_none()
                || is_generic_title(merged.title.as_deref()));

        if needs_headless && !opts.no_headless {
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
/// Tracks which fetcher provided each field in `sources`.
fn merge_results(mut results: Vec<(u8, &str, Metadata)>, scrape_config: Option<&ScrapeConfig>) -> Metadata {
    results.sort_by_key(|(priority, _, _)| *priority);

    let mut merged = Metadata::default();

    for (_priority, name, m) in &results {
        if m.title.is_some() {
            let incoming_generic = is_generic_title(m.title.as_deref());
            let current_generic = is_generic_title(merged.title.as_deref());
            // Accept if: no title yet, OR current is generic and incoming is real
            if merged.title.is_none() || (current_generic && !incoming_generic) {
                merged.title.clone_from(&m.title);
                merged.sources.insert("title".into(), name.to_string());
            }
        }
        if m.description.is_some() {
            let current_empty = is_empty_description(merged.description.as_deref());
            let incoming_empty = is_empty_description(m.description.as_deref());
            if merged.description.is_none() || (current_empty && !incoming_empty) {
                merged.description.clone_from(&m.description);
                merged.sources.insert("description".into(), name.to_string());
            }
        }
        if merged.keywords.is_none() && m.keywords.is_some() {
            merged.keywords.clone_from(&m.keywords);
            merged.sources.insert("keywords".into(), name.to_string());
        }
        if merged.canonical_url.is_none() && m.canonical_url.is_some() {
            merged.canonical_url.clone_from(&m.canonical_url);
            merged.sources.insert("canonical_url".into(), name.to_string());
        }
        if merged.icon_url.is_none() && m.icon_url.is_some() {
            merged.icon_url.clone_from(&m.icon_url);
            merged.sources.insert("icon".into(), name.to_string());
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
                    merged.image_source = Some(name.to_string());
                    merged.sources.insert("image".into(), name.to_string());
                } else {
                    log::debug!("Image from fetcher={name} failed validation, trying next source");
                }
            } else if merged.image_url.is_none() && m.image_url.is_some() {
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
                    // image_source stays from whichever fetcher provided the URL
                    if !merged.sources.contains_key("image") {
                        // Find which fetcher provided this image_url
                        for (_priority, name, m) in &results {
                            if m.image_url.as_deref() == Some(img_url.as_str()) {
                                merged.image_source = Some(name.to_string());
                                merged.sources.insert("image".into(), name.to_string());
                                break;
                            }
                        }
                    }
                }
            }
        }
    }

    merged
}

/// Merge overlay (from headless fallback) into base.
/// Fills missing fields. Additionally **overrides** generic/missing title and
/// missing description — headless often produces better text for JS-rendered pages.
fn merge_two(mut base: Metadata, overlay: Metadata, scrape_config: Option<&ScrapeConfig>) -> Metadata {
    let source = "Headless";

    // Title: override if base is missing or generic, and overlay has something better
    if overlay.title.is_some()
        && !is_generic_title(overlay.title.as_deref())
        && (base.title.is_none() || is_generic_title(base.title.as_deref()))
    {
        base.title = overlay.title;
        base.sources.insert("title".into(), source.into());
    }
    // Description: override if base is missing and overlay has one
    if base.description.is_none() && overlay.description.is_some() {
        base.description = overlay.description;
        base.sources.insert("description".into(), source.into());
    }
    if base.keywords.is_none() {
        base.keywords = overlay.keywords;
    }
    if base.canonical_url.is_none() && overlay.canonical_url.is_some() {
        base.canonical_url = overlay.canonical_url;
        base.sources.insert("canonical_url".into(), source.into());
    }
    if base.icon_url.is_none() && overlay.icon_url.is_some() {
        base.icon_url = overlay.icon_url;
        base.sources.insert("icon".into(), source.into());
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
                base.image_source = Some(source.into());
                base.sources.insert("image".into(), source.into());
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
                        base.image_source = Some(source.into());
                        base.sources.insert("image".into(), source.into());
                    }
                }
            }
        }
    }

    base
}

/// Describe which fields are present in metadata (for logging)
/// Detect titles that are site-wide defaults rather than page-specific.
/// These indicate the lightweight fetchers failed to get real content
/// (common with JS-rendered SPAs like Reddit, Twitter, etc.).
fn is_generic_title(title: Option<&str>) -> bool {
    let Some(t) = title else { return true };
    let trimmed = t.trim();
    let lower = trimmed.to_lowercase();

    // Too short to be meaningful
    if trimmed.len() < 4 {
        return true;
    }

    // Known generic site titles
    let generics = [
        "reddit - the heart of the internet",
        "reddit - dive into anything",
        "just a moment...",
        "twitter",
        "x. it's what's happening",
        "instagram",
        "tiktok",
        "loading...",
        "redirecting...",
        "403 forbidden",
        "access denied",
        "attention required!",
        "please wait...",
        "verify you are human",
        "page not found",
        "404 not found",
        "untitled",
        "home",
        "index",
    ];
    if generics.iter().any(|g| lower == *g) {
        return true;
    }

    // Prefix patterns
    if lower.starts_with("just a moment")
        || lower.starts_with("please enable")
        || lower.starts_with("you are being redirected")
        || lower.starts_with("checking your browser")
        || lower.starts_with("one moment")
    {
        return true;
    }

    // Title is just a bare domain name
    if lower.contains('.') && !lower.contains(' ') && !lower.contains('/')
        && reqwest::Url::parse(&format!("https://{}", lower)).is_ok()
    {
        return true;
    }

    // Common error/block page indicators
    if lower.contains("| 404")
        || lower.contains("| page not found")
        || lower.contains("| error")
        || lower.contains("access denied")
        || lower.contains("security check")
    {
        return true;
    }

    false
}

fn is_empty_description(desc: Option<&str>) -> bool {
    match desc {
        None => true,
        Some(d) => d.trim().is_empty() || d.trim().len() < 10,
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::metadata::types::Metadata;

    fn meta_with_title(title: &str) -> Metadata {
        Metadata {
            title: Some(title.to_string()),
            ..Default::default()
        }
    }

    #[test]
    fn test_generic_title_overridden_in_merge() {
        let results = vec![
            (1, "Plain", meta_with_title("Reddit - Dive into anything")),
            (5, "Microlink", meta_with_title("Actual Post Title")),
        ];
        let merged = merge_results(results, None);
        assert_eq!(merged.title.as_deref(), Some("Actual Post Title"));
    }

    #[test]
    fn test_generic_title_short() {
        assert!(is_generic_title(Some("Hi")));
        assert!(is_generic_title(Some("abc")));
        assert!(!is_generic_title(Some("A Real Title")));
    }

    #[test]
    fn test_generic_title_domain() {
        assert!(is_generic_title(Some("example.com")));
        assert!(is_generic_title(Some("www.reddit.com")));
        assert!(!is_generic_title(Some("example.com is great")));
    }

    #[test]
    fn test_generic_title_error_page() {
        assert!(is_generic_title(Some("Something | 404")));
        assert!(is_generic_title(Some("Oops | Page Not Found")));
        assert!(is_generic_title(Some("Site | Error")));
    }

    #[test]
    fn test_empty_description_replaced() {
        let results = vec![
            (1, "Plain", Metadata {
                description: Some("short".to_string()),
                ..Default::default()
            }),
            (5, "Microlink", Metadata {
                description: Some("A much longer and more useful description of the page".to_string()),
                ..Default::default()
            }),
        ];
        let merged = merge_results(results, None);
        assert_eq!(
            merged.description.as_deref(),
            Some("A much longer and more useful description of the page")
        );
    }

    #[test]
    fn test_real_title_not_overridden() {
        let results = vec![
            (1, "Plain", meta_with_title("Good Specific Title")),
            (5, "DDG", meta_with_title("Different Title")),
        ];
        let merged = merge_results(results, None);
        assert_eq!(merged.title.as_deref(), Some("Good Specific Title"));
    }
}
