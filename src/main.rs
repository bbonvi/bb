use clap::Parser;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod app;
mod auth;
mod bookmarks;
mod buku_migrate;
mod cli;
mod config;
mod editor;
mod eid;
mod images;
mod metadata;
mod rules;
mod scrape;
mod storage;
#[cfg(test)]
mod tests;
mod web;

use cli::{Args, Command};

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

fn main() -> anyhow::Result<()> {
    if std::env::var("RUST_LOG").is_err() {
        unsafe { std::env::set_var("RUST_LOG", "error") }
    }

    setup_logger();

    let args = Args::parse();

    match args.command {
        #[cfg(feature = "markdown-docs")]
        Command::MarkdownDocs {} => {
            let markdown: String = clap_markdown::help_markdown::<Args>();
            println!("{markdown}");
            Ok(())
        }

        Command::MigrateBuku {} => {
            buku_migrate::migrate();
            Ok(())
        }

        Command::Daemon { .. } => {
            log::debug!("Creating application manager...");
            let mut app_mgr = app::AppFactory::create_local_app()?;

            #[cfg(feature = "headless")]
            log::debug!("Testing headless chrome launch...");
            scrape::headless::test_launch();
            log::debug!("launched chrome successfully");

            log::debug!("Starting queue processor...");
            app_mgr.run_queue();
            let paths = app::AppFactory::get_paths()?;
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
            count,
            action,
        } => {
            let app_service = app::AppFactory::create_app_service()?;
            let params = cli::SearchParams {
                url,
                title,
                description,
                tags,
                keyword,
                id,
                exact,
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
            let app_service = app::AppFactory::create_app_service()?;
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

        Command::Meta { url, meta_args } => {
            cli::handle_meta(url, meta_args.no_https_upgrade, meta_args.no_headless)
        }

        Command::Rule { action } => {
            let app_service = app::AppFactory::create_app_service()?;
            let config = app_service.get_config()?;
            let mut conf = config.write().unwrap();
            cli::handle_rule(action, &mut conf)
        }
    }
}
