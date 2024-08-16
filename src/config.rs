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
    pub rules: Vec<Rule>,
}

fn task_queue_max_threads() -> u16 {
    TASK_QUEUE_MAX_THREADS
}

impl Config {
    pub fn load() -> Self {
        let store = storage::BackendLocal::new("./");

        // create new if does not exist
        if !store.exists("config.yaml") {
            let config_str = serde_yml::to_string(&Self::default()).unwrap();
            store.write("config-temp.yaml", &config_str.as_bytes());
            std::fs::rename("config-temp.yaml", "config.yaml").unwrap();
        }

        let config_str =
            String::from_utf8(store.read("config.yaml")).expect("config file is not valid utf8");
        let config: Self = serde_yml::from_str(&config_str).expect("config is malformed");

        // validate rules
        for (idx, rule) in config.rules.iter().enumerate() {
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

        // resave in case config version needs an upgrade
        if config_str != serde_yml::to_string(&config).unwrap() {
            config.save();
        }

        config
    }

    pub fn save(&self) {
        let store = storage::BackendLocal::new("./");

        let config_str = serde_yml::to_string(&self).unwrap();
        store.write("config-temp.yaml", &config_str.as_bytes());

        std::fs::rename("config-temp.yaml", "config.yaml").unwrap();
    }
}
