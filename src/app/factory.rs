use crate::{
    app::{backend::AppBackend, local::AppLocal, remote::AppRemote, service::AppService},
    config::Config,
    storage,
};
use anyhow::Result;
use homedir::my_home;
use std::sync::{Arc, RwLock};

pub struct AppFactory;

impl AppFactory {
    pub fn create_app_service() -> Result<AppService> {
        let backend = Self::create_backend()?;
        Ok(AppService::new(backend))
    }

    pub fn create_local_app() -> Result<AppLocal> {
        let (base_path, bookmarks_path, uploads_path) = Self::get_paths()?;
        
        let storage_mgr = storage::BackendLocal::new(&uploads_path);
        let config = Arc::new(RwLock::new(Config::load_with(&base_path)));
        
        Ok(AppLocal::new(config.clone(), &bookmarks_path, storage_mgr))
    }

    pub fn get_paths() -> Result<(String, String, String)> {
        let base_path = std::env::var("BB_BASE_PATH").unwrap_or_else(|_| {
            let home = my_home()
                .expect("couldn't find home dir")
                .expect("couldn't find home dir");
            format!("{}/.local/share/bb", home.to_string_lossy())
        });
        
        let bookmarks_path = format!("{base_path}/bookmarks.csv");
        let uploads_path = format!("{base_path}/uploads");
        
        std::fs::create_dir_all(&base_path).expect("couldn't create bb dir");
        
        Ok((base_path, bookmarks_path, uploads_path))
    }

    fn create_backend() -> Result<Box<dyn AppBackend>> {
        if let Ok(backend_addr) = std::env::var("BB_ADDR") {
            let basic_auth = Self::parse_basic_auth()?;
            Ok(Box::new(AppRemote::new(&backend_addr, basic_auth)))
        } else {
            let (base_path, bookmarks_path, uploads_path) = Self::get_paths()?;
            
            let config = Arc::new(RwLock::new(Config::load_with(&base_path)));
            let storage_mgr = storage::BackendLocal::new(&uploads_path);
            
            Ok(Box::new(AppLocal::new(config.clone(), &bookmarks_path, storage_mgr)))
        }
    }

    fn parse_basic_auth() -> Result<Option<(String, Option<String>)>> {
        match std::env::var("BB_BASIC_AUTH") {
            Ok(ba) => {
                let parts: Vec<_> = ba.split(":").collect();
                match parts.as_slice() {
                    [username] => Ok(Some((username.to_string(), None))),
                    [username, password] => Ok(Some((username.to_string(), Some(password.to_string())))),
                    _ => Ok(None),
                }
            }
            Err(_) => Ok(None),
        }
    }
}
