use crate::{eid::Eid, storage, storage::StorageManager};
use serde::{Deserialize, Serialize};

const WORKSPACES_FILE: &str = "workspaces.yaml";

#[derive(Clone, Debug, Serialize, Deserialize, Default, PartialEq)]
pub struct ViewPrefs {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mode: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub columns: Option<u32>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default, PartialEq)]
pub struct WorkspaceFilters {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tag_whitelist: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tag_blacklist: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url_pattern: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title_pattern: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description_pattern: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub any_field_pattern: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct Workspace {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub filters: WorkspaceFilters,
    #[serde(default)]
    pub view_prefs: ViewPrefs,
}

#[derive(Debug, thiserror::Error)]
pub enum WorkspaceError {
    #[error("workspace name must be non-empty and at most 100 characters")]
    InvalidName,
    #[error("duplicate workspace name: {0}")]
    DuplicateName(String),
    #[error("invalid regex pattern in {field}: {source}")]
    InvalidPattern {
        field: String,
        source: regex::Error,
    },
    #[error("workspace not found: {0}")]
    NotFound(String),
    #[error("storage error: {0}")]
    Storage(String),
}

pub struct WorkspaceStore {
    workspaces: Vec<Workspace>,
    base_path: String,
}

impl WorkspaceStore {
    pub fn load(base_path: &str) -> Result<Self, WorkspaceError> {
        let store = storage::BackendLocal::new(base_path);

        if !store.exists(WORKSPACES_FILE) {
            let empty: Vec<Workspace> = vec![];
            let yaml = serde_yml::to_string(&empty)
                .map_err(|e| WorkspaceError::Storage(e.to_string()))?;
            store.write(WORKSPACES_FILE, yaml.as_bytes());
        }

        let data = store.read(WORKSPACES_FILE);
        let yaml_str = String::from_utf8(data)
            .map_err(|e| WorkspaceError::Storage(e.to_string()))?;
        let workspaces: Vec<Workspace> = serde_yml::from_str(&yaml_str)
            .map_err(|e| WorkspaceError::Storage(e.to_string()))?;

        Ok(Self {
            workspaces,
            base_path: base_path.to_string(),
        })
    }

    fn save(&self) -> Result<(), WorkspaceError> {
        let store = storage::BackendLocal::new(&self.base_path);
        let yaml = serde_yml::to_string(&self.workspaces)
            .map_err(|e| WorkspaceError::Storage(e.to_string()))?;
        store.write(WORKSPACES_FILE, yaml.as_bytes());
        Ok(())
    }

    pub fn list(&self) -> &[Workspace] {
        &self.workspaces
    }

    pub fn create(
        &mut self,
        name: String,
        filters: Option<WorkspaceFilters>,
        view_prefs: Option<ViewPrefs>,
    ) -> Result<Workspace, WorkspaceError> {
        let name = validate_name(&name)?;
        let filters = filters.unwrap_or_default();
        validate_patterns(&filters)?;
        self.check_duplicate_name(&name, None)?;

        let workspace = Workspace {
            id: Eid::new().to_string(),
            name,
            filters,
            view_prefs: view_prefs.unwrap_or_default(),
        };

        self.workspaces.push(workspace.clone());
        self.save()?;
        Ok(workspace)
    }

    pub fn update(
        &mut self,
        id: &str,
        name: Option<String>,
        filters: Option<WorkspaceFilters>,
        view_prefs: Option<ViewPrefs>,
    ) -> Result<Workspace, WorkspaceError> {
        let idx = self
            .workspaces
            .iter()
            .position(|w| w.id == id)
            .ok_or_else(|| WorkspaceError::NotFound(id.to_string()))?;

        if let Some(ref name) = name {
            let name = validate_name(name)?;
            self.check_duplicate_name(&name, Some(id))?;
            self.workspaces[idx].name = name;
        }

        if let Some(filters) = filters {
            validate_patterns(&filters)?;
            self.workspaces[idx].filters = filters;
        }

        if let Some(view_prefs) = view_prefs {
            self.workspaces[idx].view_prefs = view_prefs;
        }

        self.save()?;
        Ok(self.workspaces[idx].clone())
    }

    pub fn delete(&mut self, id: &str) -> Result<(), WorkspaceError> {
        let idx = self
            .workspaces
            .iter()
            .position(|w| w.id == id)
            .ok_or_else(|| WorkspaceError::NotFound(id.to_string()))?;

        self.workspaces.remove(idx);
        self.save()?;
        Ok(())
    }

    fn check_duplicate_name(
        &self,
        name: &str,
        exclude_id: Option<&str>,
    ) -> Result<(), WorkspaceError> {
        let lower = name.to_lowercase();
        let dup = self.workspaces.iter().any(|w| {
            w.name.to_lowercase() == lower && exclude_id.map_or(true, |eid| w.id != eid)
        });
        if dup {
            return Err(WorkspaceError::DuplicateName(name.to_string()));
        }
        Ok(())
    }
}

fn validate_name(name: &str) -> Result<String, WorkspaceError> {
    let trimmed = name.trim();
    if trimmed.is_empty() || trimmed.len() > 100 {
        return Err(WorkspaceError::InvalidName);
    }
    Ok(trimmed.to_string())
}

fn validate_patterns(filters: &WorkspaceFilters) -> Result<(), WorkspaceError> {
    let patterns = [
        ("url_pattern", &filters.url_pattern),
        ("title_pattern", &filters.title_pattern),
        ("description_pattern", &filters.description_pattern),
        ("any_field_pattern", &filters.any_field_pattern),
    ];

    for (field, pattern) in patterns {
        if let Some(p) = pattern {
            regex::Regex::new(p).map_err(|e| WorkspaceError::InvalidPattern {
                field: field.to_string(),
                source: e,
            })?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    fn tmp_dir() -> String {
        let c = COUNTER.fetch_add(1, Ordering::SeqCst);
        let p = std::env::temp_dir().join(format!("bb-ws-test-{}-{}", std::process::id(), c));
        std::fs::create_dir_all(&p).unwrap();
        p.to_str().unwrap().to_string()
    }

    // -- WorkspaceStore load/save/CRUD --

    #[test]
    fn load_creates_empty_file_if_absent() {
        let dir = tmp_dir();
        let store = WorkspaceStore::load(&dir).unwrap();
        assert!(store.list().is_empty());
        // file should exist now
        assert!(std::path::Path::new(&dir).join("workspaces.yaml").exists());
    }

    #[test]
    fn load_existing_file() {
        let dir = tmp_dir();
        let yaml = r#"
- id: "abc"
  name: "Dev"
  filters: {}
  view_prefs: {}
"#;
        std::fs::write(std::path::Path::new(&dir).join("workspaces.yaml"), yaml).unwrap();
        let store = WorkspaceStore::load(&dir).unwrap();
        assert_eq!(store.list().len(), 1);
        assert_eq!(store.list()[0].name, "Dev");
    }

    #[test]
    fn create_and_reload_roundtrip() {
        let dir = tmp_dir();
        let mut store = WorkspaceStore::load(&dir).unwrap();
        let ws = store.create("Test".into(), None, None).unwrap();
        assert!(!ws.id.is_empty());
        assert_eq!(ws.name, "Test");

        // reload from disk
        let store2 = WorkspaceStore::load(&dir).unwrap();
        assert_eq!(store2.list().len(), 1);
        assert_eq!(store2.list()[0].id, ws.id);
    }

    #[test]
    fn update_existing() {
        let dir = tmp_dir();
        let mut store = WorkspaceStore::load(&dir).unwrap();
        let ws = store.create("Original".into(), None, None).unwrap();
        let updated = store
            .update(&ws.id, Some("Renamed".into()), None, None)
            .unwrap();
        assert_eq!(updated.name, "Renamed");
    }

    #[test]
    fn update_nonexistent_returns_not_found() {
        let dir = tmp_dir();
        let mut store = WorkspaceStore::load(&dir).unwrap();
        let err = store.update("nope", Some("X".into()), None, None).unwrap_err();
        assert!(matches!(err, WorkspaceError::NotFound(_)));
    }

    #[test]
    fn delete_existing() {
        let dir = tmp_dir();
        let mut store = WorkspaceStore::load(&dir).unwrap();
        let ws = store.create("ToDelete".into(), None, None).unwrap();
        store.delete(&ws.id).unwrap();
        assert!(store.list().is_empty());
    }

    #[test]
    fn delete_nonexistent_returns_not_found() {
        let dir = tmp_dir();
        let mut store = WorkspaceStore::load(&dir).unwrap();
        let err = store.delete("nope").unwrap_err();
        assert!(matches!(err, WorkspaceError::NotFound(_)));
    }

    #[test]
    fn concurrent_access_no_panic() {
        let dir = tmp_dir();
        let store = std::sync::Arc::new(std::sync::RwLock::new(
            WorkspaceStore::load(&dir).unwrap(),
        ));

        let handles: Vec<_> = (0..4)
            .map(|i| {
                let s = store.clone();
                std::thread::spawn(move || {
                    let mut guard = s.write().unwrap();
                    guard.create(format!("ws-{i}"), None, None).unwrap();
                })
            })
            .collect();

        for h in handles {
            h.join().unwrap();
        }

        let guard = store.read().unwrap();
        assert_eq!(guard.list().len(), 4);
    }

    // -- Validation --

    #[test]
    fn empty_name_rejected() {
        let dir = tmp_dir();
        let mut store = WorkspaceStore::load(&dir).unwrap();
        let err = store.create("".into(), None, None).unwrap_err();
        assert!(matches!(err, WorkspaceError::InvalidName));
    }

    #[test]
    fn whitespace_only_name_rejected() {
        let dir = tmp_dir();
        let mut store = WorkspaceStore::load(&dir).unwrap();
        let err = store.create("   ".into(), None, None).unwrap_err();
        assert!(matches!(err, WorkspaceError::InvalidName));
    }

    #[test]
    fn long_name_rejected() {
        let dir = tmp_dir();
        let mut store = WorkspaceStore::load(&dir).unwrap();
        let name = "a".repeat(101);
        let err = store.create(name, None, None).unwrap_err();
        assert!(matches!(err, WorkspaceError::InvalidName));
    }

    #[test]
    fn invalid_regex_rejected() {
        let dir = tmp_dir();
        let mut store = WorkspaceStore::load(&dir).unwrap();
        let filters = WorkspaceFilters {
            url_pattern: Some("[invalid".into()),
            ..Default::default()
        };
        let err = store.create("Valid".into(), Some(filters), None).unwrap_err();
        assert!(matches!(err, WorkspaceError::InvalidPattern { .. }));
    }

    #[test]
    fn duplicate_name_rejected_case_insensitive() {
        let dir = tmp_dir();
        let mut store = WorkspaceStore::load(&dir).unwrap();
        store.create("Dev".into(), None, None).unwrap();
        let err = store.create("dev".into(), None, None).unwrap_err();
        assert!(matches!(err, WorkspaceError::DuplicateName(_)));
    }

    #[test]
    fn valid_workspace_accepted() {
        let dir = tmp_dir();
        let mut store = WorkspaceStore::load(&dir).unwrap();
        let filters = WorkspaceFilters {
            tag_whitelist: vec!["rust".into()],
            url_pattern: Some(r"https?://.*\.rs".into()),
            ..Default::default()
        };
        let view_prefs = ViewPrefs {
            mode: Some("grid".into()),
            columns: Some(3),
        };
        let ws = store
            .create("Rust".into(), Some(filters), Some(view_prefs))
            .unwrap();
        assert_eq!(ws.name, "Rust");
        assert_eq!(ws.filters.tag_whitelist, vec!["rust"]);
        assert_eq!(ws.view_prefs.columns, Some(3));
    }

    #[test]
    fn update_same_name_on_self_allowed() {
        let dir = tmp_dir();
        let mut store = WorkspaceStore::load(&dir).unwrap();
        let ws = store.create("Keep".into(), None, None).unwrap();
        // updating the same workspace with the same name should work
        let updated = store.update(&ws.id, Some("Keep".into()), None, None).unwrap();
        assert_eq!(updated.name, "Keep");
    }
}
