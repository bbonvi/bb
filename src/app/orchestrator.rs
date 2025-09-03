use crate::{
    app::{context::{AppContext, AppContextBuilder}, factory::AppFactory, local::AppLocal},
    storage,
};
use anyhow::Result;
use std::sync::Arc;

/// Application orchestrator that manages the application lifecycle
pub struct AppOrchestrator {
    context: AppContext,
    local_app: Option<AppLocal>,
}

impl AppOrchestrator {
    /// Create a new application orchestrator
    pub fn new() -> Result<Self> {
        let environment = AppFactory::get_environment_info();
        let paths = AppFactory::get_paths()?;
        
        // Ensure directories exist
        paths.ensure_directories()?;
        
        let config = AppFactory::create_config(&paths.base_path)?;
        let storage = Arc::new(storage::BackendLocal::new(&paths.uploads_path));
        
        let service = AppFactory::create_app_service()?;
        
        let context = AppContextBuilder::new()
            .service(service)
            .config(config)
            .storage(storage)
            .environment(environment.clone())
            .build()?;
        
        let local_app = if environment.is_local() {
            Some(AppFactory::create_local_app()?)
        } else {
            None
        };
        
        Ok(Self {
            context,
            local_app,
        })
    }

    /// Get a reference to the application context
    pub fn context(&self) -> &AppContext {
        &self.context
    }

    /// Get a reference to the local app (if available)
    pub fn local_app(&self) -> Option<&AppLocal> {
        self.local_app.as_ref()
    }

    /// Get a mutable reference to the local app (if available)
    pub fn local_app_mut(&mut self) -> Option<&mut AppLocal> {
        self.local_app.as_mut()
    }

    /// Initialize the application
    pub fn initialize(&mut self) -> Result<()> {
        log::info!("Initializing application orchestrator");
        
        // Validate the context
        self.context.validate()?;
        
        // Initialize local app if available
        if let Some(local_app) = &mut self.local_app {
            Self::initialize_local_app(local_app)?;
        }
        
        log::info!("Application orchestrator initialized successfully");
        Ok(())
    }

    /// Initialize the local application
    fn initialize_local_app(local_app: &mut AppLocal) -> Result<()> {
        log::info!("Initializing local application");
        
        // Run the task queue
        local_app.run_queue();
        
        log::info!("Local application initialized successfully");
        Ok(())
    }

    /// Start the application
    pub fn start(&mut self) -> Result<()> {
        log::info!("Starting application orchestrator");
        
        // Initialize if not already done
        self.initialize()?;
        
        // Start any background services
        self.start_background_services()?;
        
        log::info!("Application orchestrator started successfully");
        Ok(())
    }

    /// Start background services
    fn start_background_services(&self) -> Result<()> {
        log::info!("Starting background services");
        
        // Add any background service initialization here
        
        Ok(())
    }

    /// Stop the application
    pub fn stop(&mut self) -> Result<()> {
        log::info!("Stopping application orchestrator");
        
        // Stop background services
        self.stop_background_services()?;
        
        // Shutdown local app if available
        if let Some(local_app) = &mut self.local_app {
            Self::shutdown_local_app(local_app)?;
        }
        
        // Shutdown context
        self.context.shutdown()?;
        
        log::info!("Application orchestrator stopped successfully");
        Ok(())
    }

    /// Stop background services
    fn stop_background_services(&self) -> Result<()> {
        log::info!("Stopping background services");
        
        // Add any background service cleanup here
        
        Ok(())
    }

    /// Shutdown the local application
    fn shutdown_local_app(local_app: &mut AppLocal) -> Result<()> {
        log::info!("Shutting down local application");
        
        // Shutdown the task queue
        local_app.shutdown();
        
        // Wait for tasks to finish
        local_app.wait_task_queue_finish();
        
        log::info!("Local application shut down successfully");
        Ok(())
    }

    /// Reload configuration
    pub fn reload_config(&self) -> Result<()> {
        log::info!("Reloading configuration");
        
        self.context.reload_config()?;
        
        log::info!("Configuration reloaded successfully");
        Ok(())
    }

    /// Get application statistics
    pub fn get_stats(&self) -> Result<crate::app::context::AppStats> {
        self.context.get_stats()
    }

    /// Check application health
    pub fn health_check(&self) -> Result<AppHealth> {
        let mut health = AppHealth::new();
        
        // Check context health
        if let Err(e) = self.context.validate() {
            health.add_issue("context", &e.to_string());
        }
        
        // Check local app health if available
        if let Some(local_app) = &self.local_app {
            if let Err(e) = Self::check_local_app_health(local_app) {
                health.add_issue("local_app", &e.to_string());
            }
        }
        
        // Check storage health
        if let Err(e) = self.check_storage_health() {
            health.add_issue("storage", &e.to_string());
        }
        
        Ok(health)
    }

    /// Check local app health
    fn check_local_app_health(_local_app: &AppLocal) -> Result<()> {
        // Add local app health checks here
        Ok(())
    }

    /// Check storage health
    fn check_storage_health(&self) -> Result<()> {
        if self.context.is_remote() {
            // Skip storage health check for remote mode
            return Ok(());
        }
        
        // Test storage write/read access
        let test_file = "health_check_test";
        let test_data = b"health_check";
        
        self.context.storage().write(test_file, test_data);
        let read_data = self.context.storage().read(test_file);
        
        if read_data != test_data {
            anyhow::bail!("Storage health check failed: data mismatch");
        }
        
        // Clean up test file
        self.context.storage().delete(test_file);
        
        Ok(())
    }

    /// Run the application with graceful shutdown
    pub fn run_with_shutdown<F>(&mut self, main_loop: F) -> Result<()>
    where
        F: FnOnce(&mut Self) -> Result<()>,
    {
        // Start the application
        self.start()?;
        
        // Set up shutdown signal handling
        let _shutdown_signal = self.setup_shutdown_signal();
        
        // Run the main loop
        let result = main_loop(self);
        
        // Ensure cleanup happens
        if let Err(ref e) = result {
            log::error!("Main loop error: {}", e);
        }
        
        // Stop the application
        self.stop()?;
        
        result
    }

    /// Set up shutdown signal handling
    fn setup_shutdown_signal(&self) -> std::sync::mpsc::Receiver<()> {
        let (tx, rx) = std::sync::mpsc::channel();
        
        // Handle Ctrl+C
        let tx_clone = tx.clone();
        ctrlc::set_handler(move || {
            log::info!("Received shutdown signal");
            let _ = tx_clone.send(());
        }).expect("Failed to set Ctrl+C handler");
        
        rx
    }
}

/// Application health status
#[derive(Debug, Clone)]
pub struct AppHealth {
    pub is_healthy: bool,
    pub issues: Vec<(String, String)>,
}

impl AppHealth {
    /// Create a new health status
    pub fn new() -> Self {
        Self {
            is_healthy: true,
            issues: Vec::new(),
        }
    }

    /// Add a health issue
    pub fn add_issue(&mut self, component: &str, message: &str) {
        self.is_healthy = false;
        self.issues.push((component.to_string(), message.to_string()));
    }

    /// Get a summary of health issues
    pub fn summary(&self) -> String {
        if self.is_healthy {
            "Application is healthy".to_string()
        } else {
            format!("Application has {} health issues", self.issues.len())
        }
    }

    /// Get detailed health report
    pub fn detailed_report(&self) -> String {
        let mut report = format!("Health Status: {}\n", self.summary());
        
        if !self.issues.is_empty() {
            report.push_str("Issues:\n");
            for (component, message) in &self.issues {
                report.push_str(&format!("  {}: {}\n", component, message));
            }
        }
        
        report
    }
}

impl Default for AppHealth {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_health() {
        let mut health = AppHealth::new();
        assert!(health.is_healthy);
        
        health.add_issue("test", "test issue");
        assert!(!health.is_healthy);
        assert_eq!(health.issues.len(), 1);
        
        let summary = health.summary();
        assert!(summary.contains("1 health issues"));
    }
}
