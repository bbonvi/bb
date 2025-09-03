use crate::{
    app::{backend::AppBackend, local::AppLocal, remote::AppRemote, service::AppService},
    config::Config,
    storage,
};
use anyhow::{Result, Context};
use homedir::my_home;
use std::sync::{Arc, RwLock};

/// Application factory for creating and configuring application components
pub struct AppFactory;

impl AppFactory {
    /// Create an application service with the appropriate backend
    pub fn create_app_service() -> Result<AppService> {
        let backend = Self::create_backend()?;
        Ok(AppService::new(backend))
    }

    /// Create a local application instance
    pub fn create_local_app() -> Result<AppLocal> {
        let paths = Self::get_paths()?;
        let storage_mgr = storage::BackendLocal::new(&paths.uploads_path);
        let config = Self::create_config(&paths.base_path)?;
        
        Ok(AppLocal::new(config.clone(), &paths.bookmarks_path, storage_mgr))
    }

    /// Create a remote application instance
    pub fn create_remote_app() -> Result<AppRemote> {
        let backend_addr = std::env::var("BB_ADDR")
            .context("BB_ADDR environment variable is required for remote mode")?;
        
        let basic_auth = Self::parse_basic_auth()?;
        Ok(AppRemote::new(&backend_addr, basic_auth))
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

    /// Create the appropriate backend based on configuration
    fn create_backend() -> Result<Box<dyn AppBackend>> {
        if let Ok(backend_addr) = std::env::var("BB_ADDR") {
            log::info!("Using remote backend: {}", backend_addr);
            let basic_auth = Self::parse_basic_auth()?;
            Ok(Box::new(AppRemote::new(&backend_addr, basic_auth)))
        } else {
            log::info!("Using local backend");
            let paths = Self::get_paths()?;
            let config = Self::create_config(&paths.base_path)?;
            let storage_mgr = storage::BackendLocal::new(&paths.uploads_path);
            
            Ok(Box::new(AppLocal::new(config.clone(), &paths.bookmarks_path, storage_mgr)))
        }
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

    /// Check if the application is running in remote mode
    pub fn is_remote_mode() -> bool {
        std::env::var("BB_ADDR").is_ok()
    }

    /// Get application environment information
    pub fn get_environment_info() -> EnvironmentInfo {
        EnvironmentInfo {
            is_remote: Self::is_remote_mode(),
            base_path: Self::get_base_path().unwrap_or_else(|_| "unknown".to_string()),
            has_basic_auth: std::env::var("BB_BASIC_AUTH").is_ok(),
            task_queue_threads: std::env::var("BB_TASK_QUEUE_THREADS")
                .ok()
                .and_then(|s| s.parse::<u16>().ok())
                .unwrap_or(4),
        }
    }
}

/// Application paths structure
#[derive(Debug, Clone)]
pub struct AppPaths {
    pub base_path: String,
    pub bookmarks_path: String,
    pub uploads_path: String,
}

impl AppPaths {
    /// Get the base path
    pub fn base_path(&self) -> &str {
        &self.base_path
    }

    /// Get the bookmarks file path
    pub fn bookmarks_path(&self) -> &str {
        &self.bookmarks_path
    }

    /// Get the uploads directory path
    pub fn uploads_path(&self) -> &str {
        &self.uploads_path
    }

    /// Ensure all required directories exist
    pub fn ensure_directories(&self) -> Result<()> {
        std::fs::create_dir_all(&self.base_path)
            .context("Failed to create base directory")?;
        
        std::fs::create_dir_all(&self.uploads_path)
            .context("Failed to create uploads directory")?;
        
        // Ensure bookmarks directory exists (for the CSV file)
        if let Some(parent) = std::path::Path::new(&self.bookmarks_path).parent() {
            std::fs::create_dir_all(parent)
                .context("Failed to create bookmarks directory")?;
        }
        
        Ok(())
    }
}

/// Environment information structure
#[derive(Debug, Clone)]
pub struct EnvironmentInfo {
    pub is_remote: bool,
    pub base_path: String,
    pub has_basic_auth: bool,
    pub task_queue_threads: u16,
}

impl EnvironmentInfo {
    /// Check if running in local mode
    pub fn is_local(&self) -> bool {
        !self.is_remote
    }

    /// Get a summary of the environment
    pub fn summary(&self) -> String {
        format!(
            "Mode: {}, Base Path: {}, Auth: {}, Task Threads: {}",
            if self.is_remote { "Remote" } else { "Local" },
            self.base_path,
            if self.has_basic_auth { "Yes" } else { "No" },
            self.task_queue_threads
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_environment_info() {
        let info = EnvironmentInfo {
            is_remote: false,
            base_path: "/test/path".to_string(),
            has_basic_auth: true,
            task_queue_threads: 8,
        };
        
        assert!(info.is_local());
        assert!(info.has_basic_auth);
        assert_eq!(info.task_queue_threads, 8);
    }

    #[test]
    fn test_app_paths() {
        let paths = AppPaths {
            base_path: "/test/base".to_string(),
            bookmarks_path: "/test/base/bookmarks.csv".to_string(),
            uploads_path: "/test/base/uploads".to_string(),
        };
        
        assert_eq!(paths.base_path(), "/test/base");
        assert_eq!(paths.bookmarks_path(), "/test/base/bookmarks.csv");
        assert_eq!(paths.uploads_path(), "/test/base/uploads");
    }
}
