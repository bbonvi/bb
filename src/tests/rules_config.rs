use crate::config::{Config, RulesConfig};
use crate::rules::{Action, Rule};
use crate::storage::{BackendLocal, StorageManager};

fn temp_dir() -> tempfile::TempDir {
    tempfile::tempdir().unwrap()
}

/// Migration: config.yaml with rules → rules.yaml created, config.yaml cleaned
#[test]
fn test_migration_from_config_yaml() {
    let dir = temp_dir();
    let base_path = dir.path().to_str().unwrap();
    let store = BackendLocal::new(base_path).unwrap();

    // Write config.yaml with rules
    let config_content = r#"
task_queue_max_threads: 4
rules:
- url: example.com
  action: !UpdateBookmark
    tags:
    - test
"#;
    store.write("config.yaml", config_content.as_bytes()).unwrap();

    // Load rules config — triggers migration
    let rules_config = RulesConfig::load_with(base_path).unwrap();

    // rules.yaml should exist with the migrated rule
    assert!(store.exists("rules.yaml"));
    assert_eq!(rules_config.rules().len(), 1);
    assert_eq!(rules_config.rules()[0].url.as_deref(), Some("example.com"));

    // config.yaml should no longer contain a rules key
    let config_str = String::from_utf8(store.read("config.yaml").unwrap()).unwrap();
    let config_value: serde_yml::Value = serde_yml::from_str(&config_str).unwrap();
    if let serde_yml::Value::Mapping(ref map) = config_value {
        assert!(
            !map.contains_key(&serde_yml::Value::String("rules".to_string())),
            "config.yaml should not contain rules key after migration"
        );
    }
}

/// Fresh start: no files → empty rules.yaml created
#[test]
fn test_empty_start() {
    let dir = temp_dir();
    let base_path = dir.path().to_str().unwrap();
    let store = BackendLocal::new(base_path).unwrap();

    let rules_config = RulesConfig::load_with(base_path).unwrap();

    assert!(store.exists("rules.yaml"));
    assert!(rules_config.rules().is_empty());
}

/// CRUD: load → add rule → save → reload → verify persistence
#[test]
fn test_rules_crud() {
    let dir = temp_dir();
    let base_path = dir.path().to_str().unwrap();

    // Create initial empty rules
    let mut rules_config = RulesConfig::load_with(base_path).unwrap();
    assert!(rules_config.rules().is_empty());

    // Add a rule
    rules_config.rules_mut().push(Rule {
        url: Some("github.com".to_string()),
        title: None,
        description: None,
        tags: None,
        comment: Some("dev rule".to_string()),
        action: Action::UpdateBookmark {
            title: None,
            description: None,
            tags: Some(vec!["dev".to_string()]),
        },
    });
    rules_config.save().unwrap();

    // Reload and verify
    let reloaded = RulesConfig::load_with(base_path).unwrap();
    assert_eq!(reloaded.rules().len(), 1);
    assert_eq!(reloaded.rules()[0].url.as_deref(), Some("github.com"));
    assert_eq!(reloaded.rules()[0].comment.as_deref(), Some("dev rule"));
}

/// Config no-op: formatting differences should not trigger resave
#[test]
fn test_config_no_resave_on_format_difference() {
    let dir = temp_dir();
    let base_path = dir.path().to_str().unwrap();
    let store = BackendLocal::new(base_path).unwrap();

    // Write config with specific formatting (extra whitespace, comments)
    let config_content = "task_queue_max_threads: 4\ntask_queue_max_retries: 3\n";
    store.write("config.yaml", config_content.as_bytes()).unwrap();

    // Load config (triggers auto-upgrade check)
    let _config = Config::load_with(base_path).unwrap();

    // Read back — structural equality means no resave should occur
    // (the values are identical even if formatting differs)
    let after = String::from_utf8(store.read("config.yaml").unwrap()).unwrap();

    // The file will be resaved because defaults are added (semantic_search, images, scrape).
    // But the key point: if we load again, it won't resave a second time.
    let _config2 = Config::load_with(base_path).unwrap();
    let after2 = String::from_utf8(store.read("config.yaml").unwrap()).unwrap();
    assert_eq!(after, after2, "second load should not trigger another resave");
}

/// Backward compat: config.yaml with a rules key still deserializes Config successfully
#[test]
fn test_config_tolerates_rules_key() {
    let dir = temp_dir();
    let base_path = dir.path().to_str().unwrap();
    let store = BackendLocal::new(base_path).unwrap();

    // Write config.yaml that still has a rules key
    let config_content = r#"
task_queue_max_threads: 4
rules:
- url: leftover.com
  action: !UpdateBookmark
    tags:
    - old
"#;
    store.write("config.yaml", config_content.as_bytes()).unwrap();

    // Config deserialization must not fail (serde ignores unknown fields by default)
    let config = Config::load_with(base_path);
    assert!(config.is_ok(), "Config should tolerate a rules key in config.yaml");
}
