pub mod types;
pub mod fetchers;

pub use types::{Metadata, MetaOptions};
pub use fetchers::FetcherRegistry;

use anyhow::Result;

/// Main entry point for fetching metadata from a URL
pub fn fetch_meta(url: &str, opts: MetaOptions) -> Result<Metadata> {
    let scrape_config = opts.scrape_config.as_ref();
    let registry = FetcherRegistry::new();

    // Try to fetch metadata using available fetchers
    if let Some(mut metadata) = registry.fetch_metadata(url, &opts)? {
        // Try to fetch any missing images/icons
        if metadata.image.is_none() {
            metadata.try_fetch_image(scrape_config);
        }
        if metadata.icon.is_none() {
            metadata.try_fetch_icon(scrape_config);
        }

        return Ok(metadata);
    }

    // If no fetcher succeeded, try fallback methods
    let mut metadata = fallback_fetch(url, &opts)?;

    // Final attempts for missing data
    if metadata.icon.is_none() {
        try_fetch_ddg_favicon(&mut metadata, url, scrape_config);
    }

    Ok(metadata)
}

/// Fallback metadata fetching when primary fetchers fail
fn fallback_fetch(url: &str, opts: &MetaOptions) -> Result<Metadata> {
    let scrape_config = opts.scrape_config.as_ref();
    // Try DuckDuckGo fallback
    if let Some(mut meta) = crate::scrape::get_data_from_ddg(url, scrape_config) {
        if meta.icon.is_none() {
            if let Some(ref icon_url) = meta.icon_url {
                if let Some(bytes) = fetchers::fetch_bytes(icon_url, scrape_config) {
                    meta.icon = Some(bytes);
                }
            }
        }
        return Ok(meta);
    }

    // If all else fails, return empty metadata
    Ok(Metadata::default())
}

/// Try to fetch favicon from DuckDuckGo if icon is missing
fn try_fetch_ddg_favicon(meta: &mut Metadata, url: &str, scrape_config: Option<&crate::config::ScrapeConfig>) {
    if meta.icon.is_none() {
        log::debug!("attempting DuckDuckGo favicon");
        if let Ok(parsed) = reqwest::Url::parse(url) {
            if let Some(host) = parsed.host_str() {
                let icon_url = format!("https://external-content.duckduckgo.com/ip3/{host}.ico");
                if let Some(bytes) = fetchers::fetch_bytes(&icon_url, scrape_config) {
                    log::debug!("favicon fetched from DuckDuckGo");
                    meta.icon = Some(bytes);
                    meta.icon_url = Some(icon_url);
                }
            }
        }
    }
}
