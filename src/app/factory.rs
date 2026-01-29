use crate::{
    app::{backend::AppBackend, local::AppLocal, remote::AppRemote, service::AppService},
    config::Config,
    semantic::SemanticSearchService,
    storage,
};
use anyhow::{Context, Result};
use homedir::my_home;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

/// Application factory for creating and configuring application components
pub struct AppFactory;

impl AppFactory {
    /// Create an application service with the appropriate backend
    ///
    /// For local backends, also creates a SemanticSearchService if enabled.
    /// For remote backends (BB_ADDR set), semantic search is handled by the daemon.
    pub fn create_app_service(paths: &AppPaths) -> Result<AppService> {
        if std::env::var("BB_ADDR").is_ok() {
            // Remote mode: semantic search handled by daemon
            let backend = Self::create_backend()?;
            Ok(AppService::new(backend))
        } else {
            // Local mode: create semantic service if config available
            let config = Self::create_config(&paths.base_path)?;

            let backend = Self::create_local_backend(&paths, config.clone())?;

            // Create semantic search service
            let semantic_config = config.read().unwrap().semantic_search.clone();
            let semantic_service = Arc::new(SemanticSearchService::new(
                semantic_config,
                PathBuf::from(&paths.base_path),
            ));

            Ok(AppService::with_semantic(backend, semantic_service))
        }
    }

    /// Create local backend with shared config
    fn create_local_backend(
        paths: &AppPaths,
        config: Arc<RwLock<Config>>,
    ) -> Result<Box<dyn AppBackend>> {
        let storage_mgr = storage::BackendLocal::new(&paths.uploads_path);
        Ok(Box::new(AppLocal::new(
            config,
            &paths.bookmarks_path,
            storage_mgr,
        )))
    }

    /// Create a local application instance
    pub fn create_local_app(paths: &AppPaths) -> Result<AppLocal> {
        let config = Arc::new(RwLock::new(Config::load_with(&paths.base_path)));
        let storage = storage::BackendLocal::new(&paths.uploads_path);

        Ok(AppLocal::new(config, &paths.bookmarks_path, storage))
    }

    /// Get application paths with validation
    pub fn get_paths() -> Result<AppPaths> {
        let base_path = Self::get_base_path()?;
        let bookmarks_path = format!("{base_path}/bookmarks.csv");
        let uploads_path = format!("{base_path}/uploads");
        
        // Ensure base directory exists
        std::fs::create_dir_all(&base_path)
            .context("Failed to create application base directory")?;
        
        Ok(AppPaths {
            base_path,
            bookmarks_path,
            uploads_path,
        })
    }

    /// Create configuration with validation
    pub fn create_config(base_path: &str) -> Result<Arc<RwLock<Config>>> {
        let config = Config::load_with(base_path);
        Self::validate_config(&config)?;
        
        Ok(Arc::new(RwLock::new(config)))
    }

    /// Get the base path for the application
    fn get_base_path() -> Result<String> {
        let base_path = std::env::var("BB_BASE_PATH").unwrap_or_else(|_| {
            let home = my_home()
                .expect("Could not determine home directory")
                .expect("Home directory path is empty");
            format!("{}/.local/share/bb", home.to_string_lossy())
        });
        
        Ok(base_path)
    }

    /// Create remote backend from BB_ADDR environment variable
    fn create_backend() -> Result<Box<dyn AppBackend>> {
        let backend_addr =
            std::env::var("BB_ADDR").context("BB_ADDR not set for remote backend")?;
        log::info!("Using remote backend: {}", backend_addr);
        let basic_auth = Self::parse_basic_auth()?;
        let bearer_token = Self::parse_bearer_token();
        Ok(Box::new(AppRemote::new(
            &backend_addr,
            basic_auth,
            bearer_token,
        )))
    }

    /// Parse basic authentication from environment
    fn parse_basic_auth() -> Result<Option<(String, Option<String>)>> {
        match std::env::var("BB_BASIC_AUTH") {
            Ok(ba) => {
                let parts: Vec<_> = ba.split(':').collect();
                match parts.as_slice() {
                    [username] => Ok(Some((username.to_string(), None))),
                    [username, password] => Ok(Some((username.to_string(), Some(password.to_string())))),
                    _ => {
                        log::warn!("Invalid BB_BASIC_AUTH format. Expected 'username' or 'username:password'");
                        Ok(None)
                    }
                }
            }
            Err(_) => Ok(None),
        }
    }

    /// Parse bearer token from environment
    fn parse_bearer_token() -> Option<String> {
        std::env::var("BB_AUTH_TOKEN")
            .ok()
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty())
    }

    /// Validate application configuration
    pub fn validate_config(config: &Config) -> Result<()> {
        if config.task_queue_max_threads == 0 {
            anyhow::bail!("Task queue max threads cannot be 0");
        }
        
        if config.task_queue_max_threads > 100 {
            anyhow::bail!("Task queue max threads cannot exceed 100");
        }
        
        // Validate rules
        for (idx, rule) in config.rules.iter().enumerate() {
            if rule.url.is_none()
                && rule.title.is_none()
                && rule.description.is_none()
                && rule.tags.is_none()
            {
                anyhow::bail!("Rule #{} is empty - at least one field must be specified", idx + 1);
            }
        }
        
        Ok(())
    }
}

/// Application paths structure
#[derive(Debug, Clone)]
pub struct AppPaths {
    pub base_path: String,
    pub bookmarks_path: String,
    pub uploads_path: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_paths() {
        let paths = AppPaths {
            base_path: "/test/base".to_string(),
            bookmarks_path: "/test/base/bookmarks.csv".to_string(),
            uploads_path: "/test/base/uploads".to_string(),
        };
        
        assert_eq!(paths.base_path, "/test/base");
        assert_eq!(paths.bookmarks_path, "/test/base/bookmarks.csv");
        assert_eq!(paths.uploads_path, "/test/base/uploads");
    }
}
