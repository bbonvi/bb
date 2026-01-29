use std::{path::PathBuf, str::FromStr};

use crate::eid::Eid;

pub trait StorageManager: Send + Sync {
    fn write(&self, ident: &str, data: &[u8]) -> std::io::Result<()>;
    fn read(&self, ident: &str) -> std::io::Result<Vec<u8>>;
    fn exists(&self, ident: &str) -> bool;
    fn delete(&self, ident: &str) -> std::io::Result<()>;
    fn list(&self) -> Vec<String>;
}

#[derive(Clone)]
pub struct BackendLocal {
    pub base_dir: PathBuf,
}

impl BackendLocal {
    pub fn new(storage_dir: &str) -> std::io::Result<Self> {
        let path = PathBuf::from_str(storage_dir)
            .expect("infallible PathBuf::from_str for &str");
        std::fs::create_dir_all(&path)?;
        Ok(BackendLocal { base_dir: path })
    }
}

impl StorageManager for BackendLocal {
    fn exists(&self, ident: &str) -> bool {
        let path = format!("{}/{ident}", &self.base_dir.to_str().unwrap());

        std::fs::metadata(&path).is_ok()
    }

    fn read(&self, ident: &str) -> std::io::Result<Vec<u8>> {
        let path = format!("{}/{ident}", &self.base_dir.to_str().unwrap());

        std::fs::read(&path)
    }

    fn write(&self, ident: &str, data: &[u8]) -> std::io::Result<()> {
        let path = format!("{}/{ident}", &self.base_dir.to_str().unwrap());
        let temp_path = format!(
            "{}/{}-{ident}",
            &self.base_dir.to_str().unwrap(),
            Eid::new()
        );

        std::fs::write(&temp_path, data)?;

        std::fs::rename(&temp_path, &path)
    }

    fn delete(&self, ident: &str) -> std::io::Result<()> {
        let path = format!("{}/{ident}", &self.base_dir.to_str().unwrap());
        std::fs::remove_file(&path)
    }

    fn list(&self) -> Vec<String> {
        std::fs::read_dir(&self.base_dir)
            .map(|entries| {
                entries
                    .filter_map(|entry| entry.ok())
                    .filter_map(|entry| {
                        let path = entry.path();
                        if path.is_file() {
                            path.file_name()
                                .and_then(|name| name.to_str())
                                .map(|s| s.to_string())
                        } else {
                            None
                        }
                    })
                    .collect()
            })
            .unwrap_or_default()
    }
}
