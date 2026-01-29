pub mod microlink;
pub mod peekalink;
pub mod plain;

use crate::config::ScrapeConfig;
use crate::metadata::types::{Metadata, MetaOptions};

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

impl FetcherRegistry {
    pub fn new() -> Self {
        let mut registry = Self { fetchers: Vec::new() };
        
        // Add fetchers in order of preference
        registry.fetchers.push(Box::new(microlink::MicrolinkFetcher::new()));
        registry.fetchers.push(Box::new(peekalink::PeekalinkFetcher::new()));
        registry.fetchers.push(Box::new(plain::PlainFetcher::new()));
        
        registry
    }
    
    /// Try all fetchers in order until one succeeds
    pub fn fetch_metadata(&self, url: &str, opts: &MetaOptions) -> anyhow::Result<Option<Metadata>> {
        let scrape_config = opts.scrape_config.as_ref();
        for fetcher in &self.fetchers {
            log::debug!("Trying fetcher: {}", fetcher.name());
            match fetcher.fetch(url, scrape_config) {
                Ok(Some(metadata)) => {
                    log::info!("Successfully fetched metadata using {}", fetcher.name());
                    return Ok(Some(metadata));
                }
                Ok(None) => {
                    log::debug!("Fetcher {} cannot handle this URL", fetcher.name());
                    continue;
                }
                Err(e) => {
                    log::warn!("Fetcher {} failed: {}", fetcher.name(), e);
                    continue;
                }
            }
        }
        
        // If no fetcher succeeded, try headless as last resort
        if !opts.no_headless {
            let headless_fetcher = plain::HeadlessFetcher::new(opts.clone());
            if let Ok(Some(metadata)) = headless_fetcher.fetch_with_headless(url, scrape_config) {
                log::info!("Successfully fetched metadata using headless browser");
                return Ok(Some(metadata));
            }
        }
        
        Ok(None)
    }
}
