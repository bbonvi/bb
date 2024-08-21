use crate::{
    rules::Rule,
    storage::{self, StorageManager},
};
use serde::{Deserialize, Serialize};

const TASK_QUEUE_MAX_THREADS: u16 = 4;
const ALLOW_DUPLICATE_BOOKMARKS: bool = false;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "task_queue_max_threads")]
    pub task_queue_max_threads: u16,
    #[serde(default = "allow_duplicate_bmarks")]
    pub allow_duplicates: bool,
    #[serde(default)]
    pub rules: Vec<Rule>,
}

fn task_queue_max_threads() -> u16 {
    TASK_QUEUE_MAX_THREADS
}
fn allow_duplicate_bmarks() -> bool {
    ALLOW_DUPLICATE_BOOKMARKS
}

impl Config {
    pub fn load() -> Self {
        Self::load_with("config")
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

    pub fn load_with(conf_name: &str) -> Self {
        let store = storage::BackendLocal::new("./");

        let path = format!("{conf_name}.yaml");

        // create new if does not exist
        if !store.exists(&path) {
            store.write(
                &path,
                &serde_yml::to_string(&Self::default()).unwrap().as_bytes(),
            );
        }

        let config_str =
            String::from_utf8(store.read(&path)).expect("config file is not valid utf8");
        let mut config: Self = serde_yml::from_str(&config_str).expect("config is malformed");

        config.validate();

        // resave in case config version needs an upgrade
        if config_str != serde_yml::to_string(&config).unwrap() {
            config.save();
        }

        config
    }

    pub fn save(&self) {
        let store = storage::BackendLocal::new("./");

        let config_str = serde_yml::to_string(&self).unwrap();
        store.write("config.yaml", &config_str.as_bytes());
    }
}
