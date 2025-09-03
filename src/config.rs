use crate::{
    rules::Rule,
    storage::{self, StorageManager},
};
use serde::{Deserialize, Serialize};

const TASK_QUEUE_MAX_THREADS: u16 = 4;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "task_queue_max_threads")]
    pub task_queue_max_threads: u16,
    #[serde(default)]
    pub hidden_by_default: Vec<String>,
    #[serde(default)]
    pub rules: Vec<Rule>,

    #[serde(skip_serializing, skip_deserializing)]
    base_path: String,
}

fn task_queue_max_threads() -> u16 {
    TASK_QUEUE_MAX_THREADS
}

impl Config {
    pub fn reload(&mut self) {
        let conf = Self::load_with(&self.base_path);

        self.task_queue_max_threads = conf.task_queue_max_threads;
        self.hidden_by_default = conf.hidden_by_default;
        self.rules = conf.rules;
    }

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
