use std::sync::{Arc, RwLock};

use anyhow::bail;
use app::{AppBackend, BmarkManagerBackend};
use clap::Parser;

mod app;
mod bookmarks;
mod buku_migrate;
mod cli;
mod config;
mod editor;
mod eid;
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

pub fn parse_tags(tags: String) -> Vec<String> {
    tags.split(',')
        .map(|value| value.split(&[' ', ' ']).filter(|value| !value.is_empty()))
        .flatten()
        .map(|s| s.to_lowercase().to_string())
        .collect::<Vec<_>>()
}

fn main() -> anyhow::Result<()> {
    let args = cli::Args::parse();

    let app_mgr = || {
        let config = Arc::new(RwLock::new(Config::load()));

        let local_backend_path = std::env::var("BB_PATH").unwrap_or(String::from("bookmarks.csv"));
        let mut backend = app::BmarkManagerBackend::Local(local_backend_path);

        if let Ok(backend_addr) = std::env::var("BB_MGR_ADDR") {
            backend = app::BmarkManagerBackend::Remote(backend_addr);
        };

        (app::AppDaemon::new(config.clone(), backend), config.clone())
    };

    match args.command {
        cli::Command::MigrateBuku {} => {
            buku_migrate::migrate();
            return Ok(());
        }

        cli::Command::Daemon { .. } => {
            let (mut app_mgr, _) = app_mgr();
            app_mgr.run_queue();
            web::start_daemon(app_mgr);
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
            meta_args:
                MetaArgs {
                    no_https_upgrade,
                    no_headless,
                    ..
                },
            ..
        } => {
            let (app_mgr, _) = app_mgr();

            handle_add(
                use_editor,
                url,
                title,
                description,
                tags,
                no_https_upgrade,
                no_headless,
                app_mgr,
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
        } => {
            let (app_mgr, _) = app_mgr();

            handle_meta(url, no_https_upgrade, no_headless, app_mgr)
        }

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
    app_mgr: app::AppDaemon,
    action: Option<ActionArgs>,
) -> anyhow::Result<()> {
    match action {
        // print results
        None => {
            println!("{}", serde_json::to_string_pretty(&bmarks).unwrap());
            Ok(())
        }

        // update results
        Some(ActionArgs::Update {
            url,
            title,
            description,
            tags,
            meta_args: _,
        }) => {
            let bmark_update = bookmarks::BookmarkUpdate {
                title,
                description,
                tags: tags.map(parse_tags),
                url,
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
            {
                println!("This update request does nothing");
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

            println!("{} items updated", count);

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

            println!("{} items removed", count);
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
    app_mgr: app::AppDaemon,
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
        println!("{}", serde_json::to_string_pretty(&bmarks).unwrap());
        return Ok(());
    }

    if count {
        println!("{} bookmarks found", bmarks.len());
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
    app_mgr: app::AppDaemon,
) -> anyhow::Result<()> {
    let mut url = url;
    let mut title = title;
    let mut description = description;
    let mut tags = tags;

    if use_editor {
        let editor_bmark = editor::edit()?;

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

    let add_opts = app::AddOpts {
        no_https_upgrade,
        async_meta: false,
        meta_opts: Some(MetaOptions { no_headless }),
    };

    let bmark = app_mgr.create(bmark_create, add_opts)?;
    println!("{}", serde_json::to_string_pretty(&bmark).unwrap());
    Ok(())
}

fn handle_meta(
    url: String,
    no_https_upgrade: bool,
    no_headless: bool,
    app_mgr: app::AppDaemon,
) -> anyhow::Result<()> {
    let fetch_meta_opts = app::FetchMetadataOpts {
        no_https_upgrade,
        meta_opts: MetaOptions { no_headless },
    };
    let meta = app::AppDaemon::fetch_metadata(&url, fetch_meta_opts)?;

    if let Some(ref image) = meta.image {
        std::fs::write("screenshot.png", &image).unwrap();
    };

    if let Some(ref icon) = meta.icon {
        std::fs::write("icon.png", &icon).unwrap();
    };

    println!("{}", serde_json::to_string_pretty(&meta).unwrap());
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
                    println!("Rule #{} // {comment}", idx + 1);
                } else {
                    println!("Rule #{}", idx + 1);
                }
                if let Some(url) = &rule.url {
                    println!("  url: {url:#?}");
                }
                if let Some(title) = &rule.title {
                    println!("  title: {title:#?}");
                }
                if let Some(description) = &rule.description {
                    println!("  description: {description:#?}");
                }
                if let Some(tags) = &rule.tags {
                    println!("  tags: {tags:#?}");
                }

                match &rule.action {
                    rules::Action::UpdateBookmark {
                        title,
                        description,
                        tags,
                    } => {
                        println!("  UpdateBookmark:");
                        if let Some(title) = &title {
                            println!("    title: {title}");
                        }
                        if let Some(description) = &description {
                            println!("    description: {description}");
                        }
                        if let Some(tags) = &tags {
                            println!("    tags: {tags:?}");
                        }
                    }
                }
                println!("");
            }
        }
    };

    Ok(())
}
