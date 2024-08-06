use clap::Parser;

mod app;
mod bookmarks;
mod cli;
mod scrape;
mod storage;
mod web;
use bookmarks::SearchQuery;
use cli::{ActionArgs, MetaArgs};
use inquire::error::InquireResult;

pub fn parse_tags(tags: String) -> Vec<String> {
    tags.split(',')
        .map(|value| value.split(&[' ', ' ']).filter(|value| !value.is_empty()))
        .flatten()
        .map(|s| s.to_string())
        .collect::<Vec<_>>()
}

fn main() {
    let args = cli::Args::parse();

    let mut app_mgr = app::App::local();

    match args.command {
        cli::Command::Daemon { .. } => {
            return web::start_daemon(app_mgr);
        }

        cli::Command::Search {
            id,
            title,
            tags,
            description,
            exact,
            url,
            no_exact_url,
            action,
            ..
        } => {
            let bookmarks = app_mgr.search(SearchQuery {
                id: id.clone(),
                title: title.clone(),
                url: url.clone(),
                description: description.clone(),
                tags: tags.clone().map(parse_tags),
                exact,
                no_exact_url,
            });

            if bookmarks.is_empty() {
                println!("not found");
                return;
            }

            match action {
                // print results
                None => {
                    println!("{}", serde_json::to_string(&bookmarks).unwrap());
                }

                // update results
                Some(ActionArgs::Update {
                    url,
                    title,
                    description,
                    tags,
                    meta_args,
                }) => {}

                // delete results
                Some(ActionArgs::Delete { yes, force }) => {
                    let is_wipe = !force
                        && url.is_none()
                        && title.is_none()
                        && description.is_none()
                        && tags.is_none()
                        && id.is_none();

                    if !is_wipe {
                        bookmarks.iter().for_each(|b| println!("{}", b.title));
                    }

                    if !yes {
                        match inquire::prompt_confirmation(
                            "Are you sure you want to delete these bookmarks?",
                        ) {
                            InquireResult::Ok(true) => {}
                            InquireResult::Ok(false) => return,
                            InquireResult::Err(err) => {
                                return println!("An error occurred: {}", err)
                            }
                        }
                    }

                    if is_wipe {
                        match inquire::prompt_confirmation(
                            "You are about to wipe your entire database. Are you really sure?",
                        ) {
                            InquireResult::Ok(true) => {}
                            InquireResult::Ok(false) => return,
                            InquireResult::Err(err) => {
                                return println!("An error occurred: {}", err)
                            }
                        }
                    }

                    for bookmark in &bookmarks {
                        app_mgr.delete(bookmark.id).unwrap();
                    }

                    println!("{} items removed", bookmarks.len())
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
            };

            match app_mgr.add(bmark_create, no_https_upgrade, no_headless, false) {
                Some(bookmark) => {
                    println!("{}", serde_json::to_string(&bookmark).unwrap());
                }
                None => {
                    println!("failed to add bookmark")
                }
            }
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
            let meta = app_mgr.fetch_metadata(&url, no_https_upgrade, no_headless);

            if let Some(meta) = meta {
                if let Some(ref image) = meta.image {
                    std::fs::write("screenshot.png", &image).unwrap();
                };

                if let Some(ref icon) = meta.icon {
                    std::fs::write("icon.png", &icon).unwrap();
                };

                println!("{}", serde_json::to_string(&meta).unwrap());
            } else {
                println!("couldn't fetch meta");
            }
        }
    }
}
