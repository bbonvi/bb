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
