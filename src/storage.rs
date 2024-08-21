use std::{path::PathBuf, str::FromStr};

pub trait StorageManager: Send + Sync {
    fn read(&self, ident: &str) -> Vec<u8>;
    fn exists(&self, ident: &str) -> bool;
    fn write(&self, ident: &str, data: &[u8]);
}

#[derive(Clone)]
pub struct BackendLocal {
    pub base_dir: PathBuf,
}

impl BackendLocal {
    pub fn new(storage_dir: &str) -> Self {
        let path = PathBuf::from_str(&storage_dir).unwrap();
        std::fs::create_dir_all(&path).unwrap();
        BackendLocal { base_dir: path }
    }
}

impl StorageManager for BackendLocal {
    fn exists(&self, ident: &str) -> bool {
        let path = format!("{}/{ident}", &self.base_dir.to_str().unwrap());

        std::fs::metadata(&path).is_ok()
    }

    fn read(&self, ident: &str) -> Vec<u8> {
        let path = format!("{}/{ident}", &self.base_dir.to_str().unwrap());

        std::fs::read(&path).unwrap()
    }

    fn write(&self, ident: &str, data: &[u8]) {
        let path = format!("{}/{ident}", &self.base_dir.to_str().unwrap());
        let temp_path = format!("{}/temp-{ident}", &self.base_dir.to_str().unwrap());

        std::fs::write(&temp_path, data).unwrap();

        std::fs::rename(&temp_path, &path).unwrap();
    }
}
