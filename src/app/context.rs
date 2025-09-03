use crate::{
    app::service::AppService,
    config::Config,
    storage,
};
use anyhow::{Result, Context};
use std::sync::{Arc, RwLock};

/// Application context that holds shared state and provides access to services
pub struct AppContext {
    /// The application service
    service: AppService,
    /// Configuration manager
    config: Arc<RwLock<Config>>,
    /// Storage manager
    storage: Arc<dyn storage::StorageManager>,
    /// Environment information
    environment: crate::app::factory::EnvironmentInfo,
}

impl AppContext {
    /// Create a new application context
    pub fn new(
        service: AppService,
        config: Arc<RwLock<Config>>,
        storage: Arc<dyn storage::StorageManager>,
        environment: crate::app::factory::EnvironmentInfo,
    ) -> Self {
        Self {
            service,
            config,
            storage,
            environment,
        }
    }

    /// Get a reference to the application service
    pub fn service(&self) -> &AppService {
        &self.service
    }

    /// Get a reference to the configuration
    pub fn config(&self) -> Arc<RwLock<Config>> {
        self.config.clone()
    }

    /// Get a reference to the storage manager
    pub fn storage(&self) -> &Arc<dyn storage::StorageManager> {
        &self.storage
    }

    /// Get environment information
    pub fn environment(&self) -> &crate::app::factory::EnvironmentInfo {
        &self.environment
    }

    /// Check if running in remote mode
    pub fn is_remote(&self) -> bool {
        self.environment.is_remote
    }

    /// Check if running in local mode
    pub fn is_local(&self) -> bool {
        self.environment.is_local()
    }

    /// Get the base path
    pub fn base_path(&self) -> &str {
        &self.environment.base_path
    }

    /// Reload configuration from disk
    pub fn reload_config(&self) -> Result<()> {
        let mut config = self.config.write().unwrap();
        let new_config = Config::load_with(self.base_path());
        *config = new_config;
        
        // Validate the new configuration
        crate::app::factory::AppFactory::validate_config(&config)?;
        
        Ok(())
    }

    /// Save configuration to disk
    pub fn save_config(&self) -> Result<()> {
        let config = self.config.read().unwrap();
        config.save();
        Ok(())
    }

    /// Get application statistics
    pub fn get_stats(&self) -> Result<AppStats> {
        let total_bookmarks = self.service.get_total_count()?;
        let tags_with_counts = self.service.get_tags_with_counts()?;
        let total_tags = tags_with_counts.len();
        
        Ok(AppStats {
            total_bookmarks,
            total_tags,
            tags_with_counts,
            environment: self.environment.clone(),
        })
    }

    /// Validate the application context
    pub fn validate(&self) -> Result<()> {
        // Validate configuration
        let config = self.config.read().unwrap();
        crate::app::factory::AppFactory::validate_config(&config)?;
        
        // Validate storage
        if !self.is_remote() {
            // Only validate storage for local mode
            self.validate_storage()?;
        }
        
        Ok(())
    }

    /// Validate storage configuration
    fn validate_storage(&self) -> Result<()> {
        // Check if storage directory is accessible
        let test_file = "test_write_access";
        let test_data = b"test";
        
        // Try to write a test file
        self.storage.write(test_file, test_data);
        
        // Check if we can read it back
        let read_data = self.storage.read(test_file);
        if read_data != test_data {
            anyhow::bail!("Storage validation failed: read data doesn't match written data");
        }
        
        // Clean up test file
        self.storage.delete(test_file);
        
        Ok(())
    }

    /// Shutdown the application context
    pub fn shutdown(&self) -> Result<()> {
        log::info!("Shutting down application context");
        
        // Save configuration
        self.save_config()?;
        
        // Additional cleanup can be added here
        
        log::info!("Application context shutdown complete");
        Ok(())
    }
}

/// Application statistics
#[derive(Debug, Clone)]
pub struct AppStats {
    pub total_bookmarks: usize,
    pub total_tags: usize,
    pub tags_with_counts: Vec<(String, usize)>,
    pub environment: crate::app::factory::EnvironmentInfo,
}

impl AppStats {
    /// Get the most popular tags
    pub fn popular_tags(&self, limit: usize) -> Vec<&(String, usize)> {
        self.tags_with_counts.iter().take(limit).collect()
    }

    /// Get tags with usage above a threshold
    pub fn frequently_used_tags(&self, min_count: usize) -> Vec<&(String, usize)> {
        self.tags_with_counts
            .iter()
            .filter(|(_, count)| *count >= min_count)
            .collect()
    }

    /// Get a summary of the statistics
    pub fn summary(&self) -> String {
        format!(
            "Bookmarks: {}, Tags: {}, Mode: {}",
            self.total_bookmarks,
            self.total_tags,
            if self.environment.is_remote { "Remote" } else { "Local" }
        )
    }
}

/// Builder for creating application contexts
pub struct AppContextBuilder {
    service: Option<AppService>,
    config: Option<Arc<RwLock<Config>>>,
    storage: Option<Arc<dyn storage::StorageManager>>,
    environment: Option<crate::app::factory::EnvironmentInfo>,
}

impl AppContextBuilder {
    /// Create a new builder
    pub fn new() -> Self {
        Self {
            service: None,
            config: None,
            storage: None,
            environment: None,
        }
    }

    /// Set the application service
    pub fn service(mut self, service: AppService) -> Self {
        self.service = Some(service);
        self
    }

    /// Set the configuration
    pub fn config(mut self, config: Arc<RwLock<Config>>) -> Self {
        self.config = Some(config);
        self
    }

    /// Set the storage manager
    pub fn storage(mut self, storage: Arc<dyn storage::StorageManager>) -> Self {
        self.storage = Some(storage);
        self
    }

    /// Set the environment information
    pub fn environment(mut self, environment: crate::app::factory::EnvironmentInfo) -> Self {
        self.environment = Some(environment);
        self
    }

    /// Build the application context
    pub fn build(self) -> Result<AppContext> {
        let service = self.service
            .context("Application service is required")?;
        
        let config = self.config
            .context("Configuration is required")?;
        
        let storage = self.storage
            .context("Storage manager is required")?;
        
        let environment = self.environment
            .context("Environment information is required")?;
        
        let context = AppContext::new(service, config, storage, environment);
        
        // Validate the context
        context.validate()?;
        
        Ok(context)
    }
}

impl Default for AppContextBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::factory::EnvironmentInfo;

    #[test]
    fn test_app_context_builder() {
        let service = AppService::new(Box::new(crate::app::local::AppLocal::new(
            Arc::new(RwLock::new(Config::default())),
            "test.csv",
            storage::BackendLocal::new("test"),
        )));
        
        let config = Arc::new(RwLock::new(Config::default()));
        let storage = Arc::new(storage::BackendLocal::new("test"));
        let environment = EnvironmentInfo {
            is_remote: false,
            base_path: "/test".to_string(),
            has_basic_auth: false,
            task_queue_threads: 4,
        };
        
        let context = AppContextBuilder::new()
            .service(service)
            .config(config)
            .storage(storage)
            .environment(environment)
            .build();
        
        assert!(context.is_ok());
    }

    #[test]
    fn test_app_stats() {
        let stats = AppStats {
            total_bookmarks: 100,
            total_tags: 25,
            tags_with_counts: vec![
                ("rust".to_string(), 50),
                ("programming".to_string(), 30),
                ("web".to_string(), 20),
            ],
            environment: EnvironmentInfo {
                is_remote: false,
                base_path: "/test".to_string(),
                has_basic_auth: false,
                task_queue_threads: 4,
            },
        };
        
        assert_eq!(stats.total_bookmarks, 100);
        assert_eq!(stats.total_tags, 25);
        assert_eq!(stats.popular_tags(2).len(), 2);
        assert_eq!(stats.frequently_used_tags(25).len(), 3);
    }
}
