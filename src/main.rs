use std::sync::{Arc, RwLock};

use anyhow::bail;
use clap::Parser;

mod app;
mod bookmarks;
mod buku_migrate;
mod cli;
mod config;
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

    let config = Arc::new(RwLock::new(Config::load()));
    let mut app_mgr = app::App::new(config.clone());

    match args.command {
        cli::Command::MigrateBuku {} => {
            buku_migrate::migrate();
            return Ok(());
        }

        cli::Command::Daemon { .. } => {
            app_mgr.run_queue();
            web::start_daemon(app_mgr);
            return Ok(());
        }

        cli::Command::Search {
            id,
            title,
            tags,
            description,
            exact,
            url,
            action,
            count,
        } => {
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
        cli::Command::Add {
            title,
            tags,
            description,
            url,
            meta_args:
                MetaArgs {
                    no_https_upgrade,
                    no_headless,
                    ..
                },
            ..
        } => {
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
            let fetch_meta_opts = app::FetchMetadataOpts {
                no_https_upgrade,
                meta_opts: MetaOptions { no_headless },
            };
            let meta = app::App::fetch_metadata(&url, fetch_meta_opts)?;

            if let Some(ref image) = meta.image {
                std::fs::write("screenshot.png", &image).unwrap();
            };

            if let Some(ref icon) = meta.icon {
                std::fs::write("icon.png", &icon).unwrap();
            };

            println!("{}", serde_json::to_string_pretty(&meta).unwrap());
            Ok(())
        }

        cli::Command::Rule { action } => {
            let mut config = config.write().unwrap();
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
    }
}
