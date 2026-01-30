use clap::Parser;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod app;
mod auth;
mod backup;
mod bookmarks;
mod cli;
mod config;
mod editor;
mod eid;
mod images;
mod lock;
mod metadata;
mod rules;
mod scrape;
mod search_query;
mod semantic;
mod storage;
#[cfg(test)]
mod tests;
mod web;
mod workspaces;

use cli::{Args, Command};
use lock::{FileLock, LockGuard};

pub fn parse_tags(tags: String) -> Vec<String> {
    tags.split(',')
        .flat_map(|value| value.split(&[' ', ' ']).filter(|value| !value.is_empty()))
        .map(|s| s.to_lowercase().to_string())
        .collect::<Vec<_>>()
}

fn setup_logger() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                // axum logs rejections from built-in extractors with the `axum::rejection`
                // target, at `TRACE` level. `axum::rejection=trace` enables showing those events
                format!(
                    "{}=debug,tower_http=debug,axum::rejection=trace",
                    env!("CARGO_CRATE_NAME")
                )
                .into()
            }),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();
}

/// Acquire database lock for CLI operations in local mode.
/// Skips if BB_ADDR is set (daemon handles locking).
fn acquire_cli_lock(base_path: &std::path::Path) -> anyhow::Result<LockGuard> {
    LockGuard::acquire_if_local(base_path).map_err(|e| {
        if e.kind() == std::io::ErrorKind::WouldBlock {
            anyhow::anyhow!(
                "Database locked. Set BB_ADDR to connect to running daemon, or stop the daemon."
            )
        } else {
            anyhow::anyhow!("Failed to acquire lock: {}", e)
        }
    })
}

fn main() -> anyhow::Result<()> {
    if std::env::var("RUST_LOG").is_err() {
        unsafe { std::env::set_var("RUST_LOG", "error") }
    }

    setup_logger();

    let args = Args::parse();
    let paths = app::AppFactory::get_paths()?;
    let base_path = std::path::Path::new(&paths.base_path);

    match args.command {
        #[cfg(feature = "markdown-docs")]
        Command::MarkdownDocs {} => {
            let markdown: String = clap_markdown::help_markdown::<Args>();
            println!("{markdown}");
            Ok(())
        }

        Command::Daemon { .. } => {
            let _lock = FileLock::try_acquire(base_path)
                .map_err(|_| anyhow::anyhow!("Another instance is running"))?;

            log::debug!("Creating application manager...");
            let mut app_mgr = app::AppFactory::create_local_app(&paths)?;

            #[cfg(feature = "headless")]
            log::debug!("Testing headless chrome launch...");
            scrape::headless::test_launch();
            log::debug!("launched chrome successfully");

            log::debug!("Starting queue processor...");
            app_mgr.run_queue();
            log::debug!("starting web server...");
            web::start_daemon(app_mgr, &paths.base_path);
            Ok(())
        }

        Command::Search {
            url,
            title,
            description,
            tags,
            keyword,
            id,
            exact,
            semantic,
            threshold,
            count,
            action,
        } => {
            let _lock = if action.as_ref().map_or(false, |a| a.is_write()) {
                Some(acquire_cli_lock(base_path)?)
            } else {
                None
            };
            let app_service = app::AppFactory::create_app_service(&paths)?;
            let params = cli::SearchParams {
                url,
                title,
                description,
                tags,
                keyword,
                id,
                exact,
                semantic,
                threshold,
                count,
                action,
            };
            cli::handle_search(params, app_service)
        }

        Command::Add {
            title,
            tags,
            description,
            url,
            editor: use_editor,
            async_meta,
            meta_args,
        } => {
            let _lock = acquire_cli_lock(base_path)?;
            let app_service = app::AppFactory::create_app_service(&paths)?;
            let params = cli::AddParams {
                use_editor,
                url,
                title,
                description,
                tags,
                no_https_upgrade: meta_args.no_https_upgrade,
                no_headless: meta_args.no_headless,
                no_meta: meta_args.no_meta,
                async_meta,
            };
            log::info!("params: {:?}", params);
            cli::handle_add(params, app_service)
        }

        Command::Meta { url, output_dir, meta_args } => {
            let scrape_config = config::Config::load_with(&paths.base_path)
                .map(|c| c.scrape)
                .ok();
            cli::handle_meta(url, meta_args.no_headless, meta_args.always_headless, scrape_config, output_dir)
        }

        Command::Rule { action } => {
            let _lock = acquire_cli_lock(base_path)?;
            let app_service = app::AppFactory::create_app_service(&paths)?;
            let rules_config = app_service.get_rules()?;
            let mut rules = rules_config.write().unwrap();
            cli::handle_rule(action, &mut rules)
        }

        Command::Compress { dry_run, yes } => {
            let _lock = acquire_cli_lock(base_path)?;
            let config = config::Config::load_with(&paths.base_path)?;
            let storage = storage::BackendLocal::new(&paths.uploads_path)?;
            let bmark_mgr = std::sync::Arc::new(
                bookmarks::BackendCsv::load(&paths.bookmarks_path)?
            );
            cli::handle_compress(dry_run, yes, &storage, bmark_mgr, &config)
        }

        Command::Backup { path } => backup::create_backup(path, base_path),

        Command::Import { path, yes } => {
            let _lock = acquire_cli_lock(base_path)?;
            backup::import_backup(path.as_deref(), yes, base_path)
        }
    }
}
