use crate::{
    rules::Rule,
    storage::{self, StorageManager},
};
use anyhow::Context;
use serde::{Deserialize, Serialize};
use serde_yml::Value;

const TASK_QUEUE_MAX_THREADS: u16 = 4;
const DEFAULT_TASK_QUEUE_MAX_RETRIES: u8 = 3;

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

    /// Always run headless Chrome in parallel with other fetchers (default: false)
    /// When false, headless only runs as a fallback when metadata is incomplete.
    #[serde(default)]
    pub always_headless: bool,

    /// Fetcher priority order. Position = priority (first = highest).
    /// Absent or empty = default order.
    #[serde(default = "default_fetcher_order")]
    pub fetcher_order: Vec<String>,
}

impl Default for ScrapeConfig {
    fn default() -> Self {
        Self {
            accept_invalid_certs: false,
            allowed_schemes: default_allowed_schemes(),
            blocked_hosts: Vec::new(),
            block_private_ips: true,
            always_headless: false,
            fetcher_order: default_fetcher_order(),
        }
    }
}

pub fn default_fetcher_order() -> Vec<String> {
    ["oEmbed", "Wayback", "Plain", "Microlink", "Peekalink", "Iframely", "DDG"]
        .iter()
        .map(|s| s.to_string())
        .collect()
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
    #[serde(default = "task_queue_max_retries")]
    pub task_queue_max_retries: u8,
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

fn task_queue_max_retries() -> u8 {
    DEFAULT_TASK_QUEUE_MAX_RETRIES
}

impl Default for Config {
    fn default() -> Self {
        Self {
            task_queue_max_threads: TASK_QUEUE_MAX_THREADS,
            task_queue_max_retries: DEFAULT_TASK_QUEUE_MAX_RETRIES,
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

        if self.task_queue_max_retries < 1 || self.task_queue_max_retries > 10 {
            errors.push(format!(
                "task_queue_max_retries must be between 1 and 10, got {}",
                self.task_queue_max_retries
            ));
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

        // resave in case config version needs an upgrade (structural comparison
        // avoids spurious resaves from formatting/comment differences)
        let original_value: Value = serde_yml::from_str(&config_str)
            .unwrap_or(Value::Null);
        let current_value: Value = serde_yml::from_str(
            &serde_yml::to_string(&config).unwrap()
        ).unwrap_or(Value::Null);
        if original_value != current_value {
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

/// Separate configuration for rules, stored in `rules.yaml`.
///
/// Rules are the only frequently machine-mutated data. Splitting them out
/// keeps `config.yaml` effectively read-only for the application, preserving
/// user comments.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RulesConfig {
    #[serde(default)]
    pub rules: Vec<Rule>,

    #[serde(skip_serializing, skip_deserializing)]
    base_path: String,
}

impl Default for RulesConfig {
    fn default() -> Self {
        Self {
            rules: Vec::new(),
            base_path: String::new(),
        }
    }
}

impl RulesConfig {
    /// Create a RulesConfig from a Vec of rules (used by remote backend).
    pub fn from_rules(rules: Vec<Rule>) -> Self {
        Self {
            rules,
            base_path: String::new(),
        }
    }

    pub fn load_with(base_path: &str) -> anyhow::Result<Self> {
        let store = storage::BackendLocal::new(base_path)
            .context("failed to initialize rules storage")?;

        if store.exists("rules.yaml") {
            // Normal path: rules.yaml exists
            let rules_str =
                String::from_utf8(store.read("rules.yaml").context("failed to read rules.yaml")?)
                    .context("rules.yaml is not valid utf8")?;
            let mut rules_config: Self =
                serde_yml::from_str(&rules_str).context("rules.yaml is malformed")?;
            rules_config.base_path = base_path.to_string();

            if let Err(errors) = rules_config.validate() {
                anyhow::bail!("rules validation failed:\n{}", errors.join("\n"));
            }

            return Ok(rules_config);
        }

        // Migration: check if config.yaml has a rules key
        if store.exists("config.yaml") {
            let config_str =
                String::from_utf8(store.read("config.yaml").context("failed to read config.yaml")?)
                    .context("config.yaml is not valid utf8")?;

            let config_value: Value =
                serde_yml::from_str(&config_str).unwrap_or(Value::Null);

            if let Value::Mapping(ref map) = config_value {
                if map.contains_key(&Value::String("rules".to_string())) {
                    // Extract rules from config.yaml
                    let rules_value = map.get(&Value::String("rules".to_string()))
                        .cloned()
                        .unwrap_or(Value::Sequence(vec![]));

                    let rules: Vec<Rule> = serde_yml::from_value(rules_value)
                        .unwrap_or_default();

                    let rules_config = Self {
                        rules,
                        base_path: base_path.to_string(),
                    };

                    // Save rules.yaml
                    rules_config.save().context("failed to write migrated rules.yaml")?;

                    // Resave config.yaml without the rules key
                    let mut new_map = map.clone();
                    new_map.remove(&Value::String("rules".to_string()));
                    let new_config_str = serde_yml::to_string(&Value::Mapping(new_map))
                        .context("failed to serialize config without rules")?;
                    store.write("config.yaml", new_config_str.as_bytes())
                        .context("failed to resave config.yaml after migration")?;

                    log::info!("migrated rules from config.yaml to rules.yaml");
                    return Ok(rules_config);
                }
            }
        }

        // Fresh start: create empty rules.yaml
        let mut rules_config = Self::default();
        rules_config.base_path = base_path.to_string();
        rules_config.save().context("failed to write default rules.yaml")?;
        Ok(rules_config)
    }

    pub fn save(&self) -> anyhow::Result<()> {
        let store = storage::BackendLocal::new(&self.base_path)
            .context("failed to initialize rules storage")?;

        let rules_str = serde_yml::to_string(&self)
            .context("failed to serialize rules config")?;
        store.write("rules.yaml", rules_str.as_bytes())
            .context("failed to write rules.yaml")?;
        Ok(())
    }

    pub fn rules(&self) -> &[Rule] {
        &self.rules
    }

    pub fn rules_mut(&mut self) -> &mut Vec<Rule> {
        &mut self.rules
    }

    pub fn validate(&self) -> Result<(), Vec<String>> {
        let mut errors = Vec::new();

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

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }
}
