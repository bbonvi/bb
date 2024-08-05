use std::{path::PathBuf, str::FromStr};

pub trait StorageBackend: Send + Sync {
    fn read(&self, ident: &str) -> Vec<u8>;
    fn write(&self, ident: &str, data: &[u8]);
}

#[derive(Clone)]
pub struct LocalStorage {
    pub base_dir: PathBuf,
}

impl LocalStorage {
    pub fn new(storage_dir: &str) -> Self {
        LocalStorage {
            base_dir: PathBuf::from_str(&storage_dir).unwrap(),
        }
    }
}

impl StorageBackend for LocalStorage {
    fn read(&self, ident: &str) -> Vec<u8> {
        std::fs::create_dir_all(&self.base_dir).unwrap();

        let path = format!("{}/{ident}", &self.base_dir.to_str().unwrap());

        std::fs::read(&path).unwrap()
    }

    fn write(&self, ident: &str, data: &[u8]) {
        std::fs::create_dir_all(&self.base_dir).unwrap();

        let path = format!("{}/{ident}", &self.base_dir.to_str().unwrap());
        let temp_path = format!("{}/temp-{ident}", &self.base_dir.to_str().unwrap());

        std::fs::write(&temp_path, data).unwrap();

        std::fs::rename(&temp_path, &path).unwrap();
    }
}
