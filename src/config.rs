use crate::{
    rules::Rule,
    storage::{self, StorageManager},
};
use serde::{Deserialize, Serialize};

const TASK_QUEUE_MAX_THREADS: u16 = 4;

/// Default semantic search model (bge-base offers +13% accuracy vs MiniLM)
const DEFAULT_SEMANTIC_MODEL: &str = "bge-base-en-v1.5";
/// Default similarity threshold for semantic search
const DEFAULT_SEMANTIC_THRESHOLD: f32 = 0.35;
/// Default model download timeout in seconds
const DEFAULT_DOWNLOAD_TIMEOUT_SECS: u64 = 300;

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

    /// Parallelism for embedding generation: "auto" or a positive integer
    #[serde(default = "default_embedding_parallelism")]
    pub embedding_parallelism: String,

    /// Timeout for model download in seconds
    #[serde(default = "default_download_timeout_secs")]
    pub download_timeout_secs: u64,
}

impl Default for SemanticSearchConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            model: DEFAULT_SEMANTIC_MODEL.to_string(),
            default_threshold: DEFAULT_SEMANTIC_THRESHOLD,
            embedding_parallelism: "auto".to_string(),
            download_timeout_secs: DEFAULT_DOWNLOAD_TIMEOUT_SECS,
        }
    }
}

fn default_semantic_model() -> String {
    DEFAULT_SEMANTIC_MODEL.to_string()
}

fn default_semantic_threshold() -> f32 {
    DEFAULT_SEMANTIC_THRESHOLD
}

fn default_embedding_parallelism() -> String {
    "auto".to_string()
}

fn default_download_timeout_secs() -> u64 {
    DEFAULT_DOWNLOAD_TIMEOUT_SECS
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "task_queue_max_threads")]
    pub task_queue_max_threads: u16,
    #[serde(default)]
    pub hidden_by_default: Vec<String>,
    #[serde(default)]
    pub rules: Vec<Rule>,
    #[serde(default)]
    pub semantic_search: SemanticSearchConfig,

    #[serde(skip_serializing, skip_deserializing)]
    base_path: String,
}

fn task_queue_max_threads() -> u16 {
    TASK_QUEUE_MAX_THREADS
}

impl Config {
    fn validate(&mut self) {
        if self.task_queue_max_threads == 0 {
            self.task_queue_max_threads = 1
        }

        // validate rules
        for (idx, rule) in self.rules.iter().enumerate() {
            if rule.url.is_none()
                && rule.title.is_none()
                && rule.description.is_none()
                && rule.tags.is_none()
            {
                let idx = idx + 1;
                panic!("rule #{idx} is empty");
            }

            Rule::is_string_matches(&rule.url.clone().unwrap_or_default(), "");
            Rule::is_string_matches(&rule.title.clone().unwrap_or_default(), "");
            Rule::is_string_matches(&rule.description.clone().unwrap_or_default(), "");
        }

        // validate semantic_search config
        let sem = &self.semantic_search;
        if !(0.0..=1.0).contains(&sem.default_threshold) {
            panic!(
                "semantic_search.default_threshold must be between 0.0 and 1.0, got {}",
                sem.default_threshold
            );
        }

        // validate embedding_parallelism: "auto" or positive integer
        if sem.embedding_parallelism != "auto" {
            match sem.embedding_parallelism.parse::<u32>() {
                Ok(0) => panic!("semantic_search.embedding_parallelism must be 'auto' or a positive integer, got '0'"),
                Err(_) => panic!(
                    "semantic_search.embedding_parallelism must be 'auto' or a positive integer, got '{}'",
                    sem.embedding_parallelism
                ),
                Ok(_) => {}
            }
        }

        if sem.download_timeout_secs == 0 {
            panic!("semantic_search.download_timeout_secs must be greater than 0");
        }
    }

    pub fn load_with(base_path: &str) -> Self {
        let store = storage::BackendLocal::new(base_path);

        // create new if does not exist
        if !store.exists("config.yaml") {
            store.write(
                "config.yaml",
                serde_yml::to_string(&Self::default()).unwrap().as_bytes(),
            );
        }

        let config_str =
            String::from_utf8(store.read("config.yaml")).expect("config file is not valid utf8");
        let mut config: Self = serde_yml::from_str(&config_str).expect("config is malformed");

        config.base_path = base_path.to_string();

        config.validate();

        // resave in case config version needs an upgrade
        if config_str != serde_yml::to_string(&config).unwrap() {
            config.save();
        }

        config
    }

    pub fn save(&self) {
        let store = storage::BackendLocal::new(&self.base_path);

        let config_str = serde_yml::to_string(&self).unwrap();
        store.write("config.yaml", config_str.as_bytes());
    }
}
