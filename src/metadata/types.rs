use crate::config::ScrapeConfig;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::Duration;

/// Error types for metadata fetching, distinguishing retryable from terminal failures
#[derive(Debug, Clone)]
pub enum FetchError {
    /// Transient failures (5xx, timeout, connection error) — worth retrying
    Retryable(String),
    /// Permanent failures (4xx, parse error, URL not supported) — do not retry
    Terminal(String),
}

impl std::fmt::Display for FetchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FetchError::Retryable(msg) => write!(f, "retryable: {msg}"),
            FetchError::Terminal(msg) => write!(f, "terminal: {msg}"),
        }
    }
}

impl std::error::Error for FetchError {}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Metadata {
    pub title: Option<String>,
    pub description: Option<String>,
    pub keywords: Option<String>,
    pub canonical_url: Option<String>,
    pub icon_url: Option<String>,
    pub image_url: Option<String>,
    #[serde(skip_serializing, skip_deserializing)]
    pub image: Option<Vec<u8>>,
    #[serde(skip_serializing, skip_deserializing)]
    pub icon: Option<Vec<u8>>,
    pub dump: Option<String>,
    /// Whether the image bytes have been validated (magic bytes, dimensions, decode)
    #[serde(skip_serializing, skip_deserializing)]
    pub image_valid: bool,
    /// Which fetcher produced the validated image
    #[serde(skip_serializing, skip_deserializing)]
    pub image_source: Option<String>,
    /// Which fetcher provided each field (field_name → fetcher_name)
    #[serde(skip_serializing, skip_deserializing)]
    pub sources: HashMap<String, String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MetaOptions {
    pub no_headless: bool,
    #[serde(skip)]
    pub scrape_config: Option<ScrapeConfig>,
    /// Per-fetcher timeout budget
    #[serde(skip)]
    pub fetcher_timeout: Option<Duration>,
}

impl Metadata {
    /// Returns true if this metadata has a validated image
    pub fn has_valid_image(&self) -> bool {
        self.image.is_some() && self.image_valid
    }

    /// Returns true if any useful field is present
    pub fn has_any_data(&self) -> bool {
        self.title.is_some()
            || self.description.is_some()
            || self.image.is_some()
            || self.icon.is_some()
            || self.image_url.is_some()
            || self.icon_url.is_some()
            || self.canonical_url.is_some()
    }

    /// Try to fetch and set image bytes from image_url if present
    pub fn try_fetch_image(&mut self, scrape_config: Option<&ScrapeConfig>) {
        if let Some(ref img_url) = self.image_url {
            if let Some(bytes) = crate::metadata::fetchers::fetch_bytes(img_url, scrape_config) {
                self.image = Some(bytes);
            }
        }
    }

    /// Try to fetch and set icon bytes from icon_url if present
    pub fn try_fetch_icon(&mut self, scrape_config: Option<&ScrapeConfig>) {
        if let Some(ref icon_url) = self.icon_url {
            if let Some(bytes) = crate::metadata::fetchers::fetch_bytes(icon_url, scrape_config) {
                self.icon = Some(bytes);
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MetadataReport {
    pub fetchers: Vec<FetcherReport>,
    pub field_decisions: Vec<FieldDecision>,
    pub headless_fallback: Option<HeadlessFallbackInfo>,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FetcherReport {
    pub name: String,
    pub priority: u8,
    pub status: FetcherStatus,
    pub duration_ms: u64,
    pub fields: Option<FetcherFields>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status", content = "detail")]
pub enum FetcherStatus {
    Success,
    Skip,
    Error(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FetcherFields {
    pub title: Option<String>,
    pub description: Option<String>,
    pub keywords: Option<String>,
    pub canonical_url: Option<String>,
    pub image_url: Option<String>,
    pub icon_url: Option<String>,
    pub has_image_bytes: bool,
    pub has_icon_bytes: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FieldDecision {
    pub field: String,
    pub winner: String,
    pub reason: String,
    pub value_preview: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeadlessFallbackInfo {
    pub triggered: bool,
    pub reason: String,
    pub status: FetcherStatus,
    pub fields_overridden: Vec<String>,
}
