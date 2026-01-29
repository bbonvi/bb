use crate::{
    rules::Rule,
    storage::{self, StorageManager},
};
use anyhow::Context;
use serde::{Deserialize, Serialize};

const TASK_QUEUE_MAX_THREADS: u16 = 4;

/// Default semantic search model
const DEFAULT_SEMANTIC_MODEL: &str = "all-MiniLM-L6-v2";
/// Default similarity threshold for semantic search
const DEFAULT_SEMANTIC_THRESHOLD: f32 = 0.35;
/// Default model download timeout in seconds
const DEFAULT_DOWNLOAD_TIMEOUT_SECS: u64 = 300;
/// Default semantic weight in hybrid search (0.0-1.0, higher = favor semantic over lexical)
const DEFAULT_SEMANTIC_WEIGHT: f32 = 0.6;

/// Configuration for URL scraping/fetching behavior
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ScrapeConfig {
    /// Accept invalid TLS certificates (default: false)
    #[serde(default)]
    pub accept_invalid_certs: bool,

    /// Allowed URL schemes for fetching (default: ["http", "https"])
    #[serde(default = "default_allowed_schemes")]
    pub allowed_schemes: Vec<String>,

    /// Blocked hostnames (default: [])
    #[serde(default)]
    pub blocked_hosts: Vec<String>,

    /// Block requests to private/loopback IP ranges (default: true)
    #[serde(default = "default_block_private_ips")]
    pub block_private_ips: bool,
}

impl Default for ScrapeConfig {
    fn default() -> Self {
        Self {
            accept_invalid_certs: false,
            allowed_schemes: default_allowed_schemes(),
            blocked_hosts: Vec::new(),
            block_private_ips: true,
        }
    }
}

fn default_allowed_schemes() -> Vec<String> {
    vec!["http".to_string(), "https".to_string()]
}

fn default_block_private_ips() -> bool {
    true
}

/// Default max dimension for image compression
const DEFAULT_IMAGE_MAX_SIZE: u32 = 600;
/// Default WebP quality for image compression
const DEFAULT_IMAGE_QUALITY: u8 = 85;

/// Configuration for semantic search functionality
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SemanticSearchConfig {
    /// Enable or disable semantic search
    #[serde(default)]
    pub enabled: bool,

    /// Model name for embeddings (e.g., "all-MiniLM-L6-v2")
    #[serde(default = "default_semantic_model")]
    pub model: String,

    /// Default similarity threshold [0.0, 1.0]
    #[serde(default = "default_semantic_threshold")]
    pub default_threshold: f32,

    /// Timeout for model download in seconds
    #[serde(default = "default_download_timeout_secs")]
    pub download_timeout_secs: u64,

    /// Weight for semantic ranking in hybrid search [0.0, 1.0]
    /// Higher values favor semantic (embedding) similarity over lexical (keyword) matching.
    /// Default: 0.6 (60% semantic, 40% lexical)
    #[serde(default = "default_semantic_weight")]
    pub semantic_weight: f32,
}

impl Default for SemanticSearchConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            model: DEFAULT_SEMANTIC_MODEL.to_string(),
            default_threshold: DEFAULT_SEMANTIC_THRESHOLD,
            download_timeout_secs: DEFAULT_DOWNLOAD_TIMEOUT_SECS,
            semantic_weight: DEFAULT_SEMANTIC_WEIGHT,
        }
    }
}

fn default_semantic_model() -> String {
    DEFAULT_SEMANTIC_MODEL.to_string()
}

fn default_semantic_threshold() -> f32 {
    DEFAULT_SEMANTIC_THRESHOLD
}

fn default_download_timeout_secs() -> u64 {
    DEFAULT_DOWNLOAD_TIMEOUT_SECS
}

fn default_semantic_weight() -> f32 {
    DEFAULT_SEMANTIC_WEIGHT
}

/// Configuration for image compression
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ImageConfig {
    /// Maximum dimension (width or height) for preview images
    #[serde(default = "default_image_max_size")]
    pub max_size: u32,

    /// WebP quality for lossy compression (1-100)
    #[serde(default = "default_image_quality")]
    pub quality: u8,
}

impl Default for ImageConfig {
    fn default() -> Self {
        Self {
            max_size: DEFAULT_IMAGE_MAX_SIZE,
            quality: DEFAULT_IMAGE_QUALITY,
        }
    }
}

fn default_image_max_size() -> u32 {
    DEFAULT_IMAGE_MAX_SIZE
}

fn default_image_quality() -> u8 {
    DEFAULT_IMAGE_QUALITY
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "task_queue_max_threads")]
    pub task_queue_max_threads: u16,
    #[serde(default)]
    pub rules: Vec<Rule>,
    #[serde(default)]
    pub semantic_search: SemanticSearchConfig,
    #[serde(default)]
    pub images: ImageConfig,
    #[serde(default)]
    pub scrape: ScrapeConfig,

    #[serde(skip_serializing, skip_deserializing)]
    base_path: String,
}

fn task_queue_max_threads() -> u16 {
    TASK_QUEUE_MAX_THREADS
}

impl Default for Config {
    fn default() -> Self {
        Self {
            task_queue_max_threads: TASK_QUEUE_MAX_THREADS,
            rules: Vec::new(),
            semantic_search: SemanticSearchConfig::default(),
            images: ImageConfig::default(),
            scrape: ScrapeConfig::default(),
            base_path: String::new(),
        }
    }
}

impl Config {
    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();

        if self.task_queue_max_threads == 0 {
            errors.push("task_queue_max_threads must be greater than 0".to_string());
        } else if self.task_queue_max_threads > 100 {
            errors.push(format!(
                "task_queue_max_threads cannot exceed 100, got {}",
                self.task_queue_max_threads
            ));
        }

        // validate rules
        for (idx, rule) in self.rules.iter().enumerate() {
            if rule.url.is_none()
                && rule.title.is_none()
                && rule.description.is_none()
                && rule.tags.is_none()
            {
                let idx = idx + 1;
                errors.push(format!("rule #{idx} is empty"));
            }

            Rule::is_string_matches(&rule.url.clone().unwrap_or_default(), "");
            Rule::is_string_matches(&rule.title.clone().unwrap_or_default(), "");
            Rule::is_string_matches(&rule.description.clone().unwrap_or_default(), "");
        }

        // validate semantic_search config
        let sem = &self.semantic_search;
        if !(0.0..=1.0).contains(&sem.default_threshold) {
            errors.push(format!(
                "semantic_search.default_threshold must be between 0.0 and 1.0, got {}",
                sem.default_threshold
            ));
        }

        if sem.download_timeout_secs == 0 {
            errors.push("semantic_search.download_timeout_secs must be greater than 0".to_string());
        }

        if !(0.0..=1.0).contains(&sem.semantic_weight) {
            errors.push(format!(
                "semantic_search.semantic_weight must be between 0.0 and 1.0, got {}",
                sem.semantic_weight
            ));
        }

        // validate images config
        let img = &self.images;
        if img.max_size == 0 || img.max_size > 4096 {
            errors.push(format!(
                "images.max_size must be between 1 and 4096, got {}",
                img.max_size
            ));
        }
        if img.quality == 0 || img.quality > 100 {
            errors.push(format!(
                "images.quality must be between 1 and 100, got {}",
                img.quality
            ));
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    pub fn load_with(base_path: &str) -> anyhow::Result<Self> {
        let store = storage::BackendLocal::new(base_path)
            .context("failed to initialize config storage")?;

        // create new if does not exist
        if !store.exists("config.yaml") {
            store.write(
                "config.yaml",
                serde_yml::to_string(&Self::default()).unwrap().as_bytes(),
            ).context("failed to write default config")?;
        }

        let config_str =
            String::from_utf8(store.read("config.yaml").context("failed to read config file")?)
                .context("config file is not valid utf8")?;
        let mut config: Self =
            serde_yml::from_str(&config_str).context("config is malformed")?;

        config.base_path = base_path.to_string();

        if let Err(errors) = config.validate() {
            anyhow::bail!("config validation failed:\n{}", errors.join("\n"));
        }

        // resave in case config version needs an upgrade
        if config_str != serde_yml::to_string(&config).unwrap() {
            config.save()?;
        }

        Ok(config)
    }

    pub fn save(&self) -> anyhow::Result<()> {
        let store = storage::BackendLocal::new(&self.base_path)
            .context("failed to initialize config storage")?;

        let config_str = serde_yml::to_string(&self)
            .context("failed to serialize config")?;
        store.write("config.yaml", config_str.as_bytes())
            .context("failed to write config file")?;
        Ok(())
    }
}
