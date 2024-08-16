use crate::{
    rules::Rule,
    storage::{self, StorageMgrBackend},
};
use serde::{Deserialize, Serialize};

const TASK_QUEUE_MAX_THREADS: u16 = 6;

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Config {
    #[serde(default = "task_queue_max_threads")]
    pub task_queue_max_threads: u16,
    pub rules: Vec<Rule>,
}

fn task_queue_max_threads() -> u16 {
    TASK_QUEUE_MAX_THREADS
}

impl Config {
    pub fn load() -> Self {
        let store = storage::StorageMgrLocal::new("./");
        if !store.exists("config.yaml") {
            let config = Self::default();
            let config_str = serde_yml::to_string(&config).unwrap();
            store.write("config.yaml", &config_str.as_bytes());
        }

        let config_str =
            String::from_utf8(store.read("config.yaml")).expect("config file is not valid utf8");
        let config: Self = serde_yml::from_str(&config_str).expect("config is malformed");

        config
    }

    pub fn save(&self) {
        let store = storage::StorageMgrLocal::new("./");

        let config_str = serde_yml::to_string(&self).unwrap();
        store.write("config.yaml", &config_str.as_bytes());
    }
}
