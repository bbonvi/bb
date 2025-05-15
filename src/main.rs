use std::sync::{Arc, RwLock};

use anyhow::bail;
use app::backend::AppBackend;
use clap::Parser;
use editor::EditorDefaults;
use std::io::Write;

mod app;
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
use bookmarks::SearchQuery;
use cli::{ActionArgs, MetaArgs, RulesArgs};
use config::Config;
use inquire::error::InquireResult;
use metadata::MetaOptions;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

pub fn parse_tags(tags: String) -> Vec<String> {
    tags.split(',')
        .flat_map(|value| value.split(&[' ', ' ']).filter(|value| !value.is_empty()))
        .map(|s| s.to_lowercase().to_string())
        .collect::<Vec<_>>()
}

// built-in println produces end of pipe error if piped to head and such
macro_rules! println {
    () => (print!("\n"));
    ($fmt:expr) => ({
        writeln!(std::io::stdout(), $fmt)
    });
    ($fmt:expr, $($arg:tt)*) => ({
        writeln!(std::io::stdout(), $fmt, $($arg)*)
    })
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
    use homedir::my_home;

    if std::env::var("RUST_LOG").is_err() {
        unsafe { std::env::set_var("RUST_LOG", "error") }
    }

    setup_logger();

    let base_path = std::env::var("BB_BASE_PATH").unwrap_or(format!(
        "{}/.local/share/bb",
        my_home()
            .expect("couldnt find home dir")
            .expect("couldnt find home dir")
            .to_string_lossy()
    ));
    let bookmarks_path = format!("{base_path}/bookmarks.csv");
    let uploads_path = format!("{base_path}/uploads");

    std::fs::create_dir_all(&base_path).expect("couldn't create bb dir");

    let args = cli::Args::parse();

    let app_local = || {
        let config = Arc::new(RwLock::new(Config::load_with(&base_path)));
        let storage_mgr = storage::BackendLocal::new(&uploads_path);
        (
            app::AppLocal::new(config.clone(), &bookmarks_path, storage_mgr),
            config,
        )
    };

    let app_mgr = || -> (Box<dyn AppBackend>, Arc<RwLock<Config>>) {
        if let Ok(backend_addr) = std::env::var("BB_ADDR") {
            let basic_auth = match std::env::var("BB_BASIC_AUTH") {
                Ok(ba) => {
                    if let Some(username) = ba.split(":").collect::<Vec<_>>().get(0) {
                        let collect = &ba.split(":").collect::<Vec<_>>();
                        let password = collect.get(1);

                        Some((username.to_string(), password.map(|p| p.to_string())))
                    } else {
                        None
                    }
                }
                Err(_) => None,
            };

            let config = Arc::new(RwLock::new(Config::load_with(&base_path)));
            (
                Box::new(app::AppRemote::new(&backend_addr, basic_auth)),
                config,
            )
        } else {
            let config = Arc::new(RwLock::new(Config::load_with(&base_path)));
            let storage_mgr = storage::BackendLocal::new(&uploads_path);
            (
                Box::new(app::AppLocal::new(
                    config.clone(),
                    &bookmarks_path,
                    storage_mgr,
                )),
                config,
            )
        }
    };

    match args.command {
        #[cfg(feature = "markdown-docs")]
        cli::Command::MarkdownDocs {} => {
            let args = cli::Args::parse();
            let markdown: String = clap_markdown::help_markdown::<cli::Args>();
            println!("{markdown}");
            Ok(())
        }

        cli::Command::MigrateBuku {} => {
            buku_migrate::migrate();
            return Ok(());
        }

        cli::Command::Daemon { .. } => {
            let (mut app_mgr, _) = app_local();

            #[cfg(feature = "headless")]
            scrape::headless::test_launch();

            app_mgr.run_queue();
            web::start_daemon(app_mgr, &base_path);
            return Ok(());
        }

        cli::Command::Search {
            url,
            title,
            description,
            tags,
            id,
            exact,
            count,
            action,
        } => {
            let (app_mgr, _) = app_mgr();

            handle_search(
                url,
                title,
                description,
                tags,
                id,
                exact,
                count,
                action,
                app_mgr,
            )
        }
        cli::Command::Add {
            title,
            tags,
            description,
            url,
            editor: use_editor,
            async_meta,
            meta_args:
                MetaArgs {
                    no_https_upgrade,
                    no_headless,
                    no_meta,
                },
            ..
        } => {
            let (app_mgr, config) = app_mgr();

            let c = config.read().unwrap();

            handle_add(
                use_editor,
                url,
                title,
                description,
                tags,
                no_https_upgrade,
                no_headless,
                no_meta,
                async_meta,
                app_mgr,
                c.clone(),
            )
        }
        cli::Command::Meta {
            url,
            meta_args:
                MetaArgs {
                    no_https_upgrade,
                    no_headless,
                    ..
                },
            ..
        } => handle_meta(url, no_https_upgrade, no_headless),

        cli::Command::Rule { action } => {
            let (_, config) = app_mgr();

            let mut config = config.write().unwrap();

            handle_rule(action, &mut config)
        }
    }
}

fn handle_action(
    bmarks: Vec<bookmarks::Bookmark>,
    query: SearchQuery,
    app_mgr: Box<dyn AppBackend>,
    action: Option<ActionArgs>,
) -> anyhow::Result<()> {
    match action {
        // print results
        None => {
            let _ = println!("{}", serde_json::to_string_pretty(&bmarks).unwrap());
            Ok(())
        }

        // update results
        Some(ActionArgs::Update {
            url,
            title,
            description,
            tags,
            append_tags,
            remove_tags,
        }) => {
            let bmark_update = bookmarks::BookmarkUpdate {
                title,
                description,
                tags: tags.map(parse_tags),
                url,
                append_tags: append_tags.map(parse_tags),
                remove_tags: remove_tags.map(parse_tags),
                ..Default::default()
            };

            let is_entire_db = query.url.is_none()
                && query.title.is_none()
                && query.description.is_none()
                && query.tags.is_none()
                && query.id.is_none();

            if bmark_update.title.is_none()
                && bmark_update.description.is_none()
                && bmark_update.tags.is_none()
                && bmark_update.url.is_none()
                && bmark_update.remove_tags.is_none()
                && bmark_update.append_tags.is_none()
            {
                let _ = println!("The update did nothing");
                return Ok(());
            }

            if is_entire_db {
                match inquire::prompt_confirmation(
                            format!("You are about to update every single bookmark ({} items). Are you really sure?", bmarks.len()),
                        ) {
                            InquireResult::Ok(true) => {}
                            InquireResult::Ok(false) => return Ok(()),
                            InquireResult::Err(err) => bail!("An error occurred: {}", err),
                        }
            }

            let count = app_mgr.search_update(query, bmark_update).unwrap();

            let _ = println!("{} items updated", count);

            Ok(())
        }

        // delete results
        Some(ActionArgs::Delete { yes, force }) => {
            let is_wipe = !force
                && query.url.is_none()
                && query.title.is_none()
                && query.description.is_none()
                && query.tags.is_none()
                && query.id.is_none();

            if !yes {
                match inquire::prompt_confirmation(format!(
                    "Are you sure you want to delete {} bookmarks?",
                    bmarks.len()
                )) {
                    InquireResult::Ok(true) => {}
                    InquireResult::Ok(false) => return Ok(()),
                    InquireResult::Err(err) => bail!("An error occurred: {}", err),
                }
            }

            if is_wipe {
                match inquire::prompt_confirmation(
                    "You are about to wipe your entire database. Are you really sure?",
                ) {
                    InquireResult::Ok(true) => {}
                    InquireResult::Ok(false) => return Ok(()),
                    InquireResult::Err(err) => bail!("An error occurred: {}", err),
                }
            }

            let count = app_mgr.search_delete(query).unwrap();

            let _ = println!("{} items removed", count);
            Ok(())
        }
    }
}

fn handle_search(
    url: Option<String>,
    title: Option<String>,
    description: Option<String>,
    tags: Option<String>,
    id: Option<u64>,
    exact: bool,
    count: bool,
    action: Option<ActionArgs>,
    app_mgr: Box<dyn AppBackend>,
) -> anyhow::Result<()> {
    let query = SearchQuery {
        id: id.clone(),
        title: title.clone(),
        url: url.clone(),
        description: description.clone(),
        tags: tags.clone().map(parse_tags),
        exact,
        limit: None,
    };
    let bmarks = app_mgr.search(query.clone())?;

    if bmarks.is_empty() {
        if action.is_some() {
            let _ = println!("0 items updated");
            return Ok(());
        }
        let _ = println!("{}", serde_json::to_string_pretty(&bmarks).unwrap());
        return Ok(());
    }

    if count {
        let _ = println!("{} bookmarks found", bmarks.len());
        return Ok(());
    }

    handle_action(bmarks, query, app_mgr, action)
}

fn handle_add(
    use_editor: bool,
    url: Option<String>,
    title: Option<String>,
    description: Option<String>,
    tags: Option<String>,
    no_https_upgrade: bool,
    no_headless: bool,
    no_meta: bool,
    async_meta: bool,
    app_mgr: Box<dyn AppBackend>,
    config: Config,
) -> anyhow::Result<()> {
    let mut url = url;
    let mut title = title;
    let mut description = description;
    let mut tags = tags;

    if use_editor {
        let mut current_tags = app_mgr.tags().unwrap();
        //let hidden = config.hidden_by_default.clone();
        current_tags.sort();

        let mut editor_defaults = EditorDefaults {
            url: url.clone(),
            title: title.clone(),
            description: description.clone(),
            tags: tags.clone(),
            current_tags,
        };

        let rules = config.rules.clone();
        if let Some(u) = url {
            for rule in rules.iter() {
                // recreating query because it could've been changed by previous rule
                let record = rules::Record {
                    url: u.clone(),
                    title: title.clone(),
                    description: description.clone(),
                    tags: tags.clone().map(parse_tags),
                };

                if !rule.is_match(&record) {
                    continue;
                }

                match &rule.action {
                    crate::rules::Action::UpdateBookmark {
                        title,
                        description,
                        tags,
                    } => {
                        if let Some(title) = title {
                            editor_defaults.title = Some(title.clone());
                        }
                        if let Some(description) = description {
                            editor_defaults.description = Some(description.clone());
                        }
                        if let Some(tags) = tags {
                            let mut curr_tags = (editor_defaults.tags.map(parse_tags))
                                .take()
                                .unwrap_or_default();
                            curr_tags.append(&mut tags.clone());
                            editor_defaults.tags = Some(curr_tags.join(" "));
                        }
                    }
                }
            }
        }

        println!("{editor_defaults:?}");

        let editor_bmark = editor::edit(editor_defaults)?;

        url = Some(editor_bmark.url);
        if let editor::EditorValue::Set(value) = editor_bmark.title {
            title = Some(value)
        }

        if let editor::EditorValue::Set(value) = editor_bmark.description {
            description = Some(value)
        }

        if let editor::EditorValue::Set(value) = editor_bmark.tags {
            tags = Some(value)
        }
    } else {
        if url.is_none() {
            anyhow::bail!("url cannot be empty");
        }
    }

    let url = url.unwrap_or_default();

    let bmark_create = bookmarks::BookmarkCreate {
        title,
        description,
        tags: tags.map(parse_tags),
        url,
        ..Default::default()
    };

    let add_opts = app::backend::AddOpts {
        no_https_upgrade,
        async_meta,
        meta_opts: if no_meta {
            None
        } else {
            Some(MetaOptions { no_headless })
        },
        skip_rules: false,
    };

    let bmark = app_mgr.create(bmark_create, add_opts)?;
    let _ = println!("{}", serde_json::to_string_pretty(&bmark).unwrap());
    Ok(())
}

fn handle_meta(url: String, no_https_upgrade: bool, no_headless: bool) -> anyhow::Result<()> {
    let fetch_meta_opts = app::backend::FetchMetadataOpts {
        no_https_upgrade,
        meta_opts: MetaOptions { no_headless },
    };

    let meta = app::AppLocal::fetch_metadata(&url, fetch_meta_opts)?;

    if let Some(ref image) = meta.image {
        std::fs::write("screenshot.png", &image).unwrap();
    };

    if let Some(ref icon) = meta.icon {
        std::fs::write("icon.png", &icon).unwrap();
    };

    let _ = println!("{}", serde_json::to_string_pretty(&meta).unwrap());
    Ok(())
}

fn handle_rule(action: RulesArgs, config: &mut Config) -> anyhow::Result<()> {
    match action {
        RulesArgs::Add {
            url,
            title,
            description,
            tags,
            action,
        } => match action {
            cli::RuleAction::Update {
                title: update_title,
                description: update_description,
                tags: update_tags,
            } => {
                let rule = rules::Rule {
                    url: url.map(|u| u.to_lowercase()),
                    description: description.map(|d| d.to_lowercase()),
                    title: title.map(|d| d.to_lowercase()),
                    tags: tags.clone().map(parse_tags),
                    action: rules::Action::UpdateBookmark {
                        title: update_title.map(|u| u.to_lowercase()),
                        description: update_description.map(|d| d.to_lowercase()),
                        tags: update_tags.clone().map(parse_tags),
                    },
                    comment: None,
                };
                config.rules.insert(0, rule);
                config.save();
            }
        },
        RulesArgs::Delete {} => todo!(),
        RulesArgs::List {} => {
            for (idx, rule) in config.rules.iter().enumerate() {
                if let Some(comment) = &rule.comment {
                    let _ = println!("Rule #{} // {comment}", idx + 1);
                } else {
                    let _ = println!("Rule #{}", idx + 1);
                }
                if let Some(url) = &rule.url {
                    let _ = println!("  url: {url:#?}");
                }
                if let Some(title) = &rule.title {
                    let _ = println!("  title: {title:#?}");
                }
                if let Some(description) = &rule.description {
                    let _ = println!("  description: {description:#?}");
                }
                if let Some(tags) = &rule.tags {
                    let _ = println!("  tags: {tags:#?}");
                }

                match &rule.action {
                    rules::Action::UpdateBookmark {
                        title,
                        description,
                        tags,
                    } => {
                        let _ = println!("  UpdateBookmark:");
                        if let Some(title) = &title {
                            let _ = println!("    title: {title}");
                        }
                        if let Some(description) = &description {
                            let _ = println!("    description: {description}");
                        }
                        if let Some(tags) = &tags {
                            let _ = println!("    tags: {tags:?}");
                        }
                    }
                }
                let _ = println!("");
            }
        }
    };

    Ok(())
}
