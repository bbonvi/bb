use crate::{
    app::backend::AppBackend,
    bookmarks::{BookmarkCreate, SearchQuery},
    config::Config,
    metadata::MetaOptions,
    parse_tags,
    rules,
};
use anyhow::{bail, Result};
use inquire::error::InquireResult;

use super::types::ActionArgs;

pub fn handle_search(
    url: Option<String>,
    title: Option<String>,
    description: Option<String>,
    tags: Option<String>,
    id: Option<u64>,
    exact: bool,
    count: bool,
    action: Option<ActionArgs>,
    app_mgr: Box<dyn AppBackend>,
) -> Result<()> {
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
            println!("0 items updated");
            return Ok(());
        }
        println!("{}", serde_json::to_string_pretty(&bmarks).unwrap());
        return Ok(());
    }

    if count {
        println!("{} bookmarks found", bmarks.len());
        return Ok(());
    }

    handle_action(bmarks, query, app_mgr, action)
}

pub fn handle_add(
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
) -> Result<()> {
    let mut url = url;
    let mut title = title;
    let mut description = description;
    let mut tags = tags;

    if use_editor {
        let mut current_tags = app_mgr.tags()?;
        current_tags.sort();

        let mut editor_defaults = crate::editor::EditorDefaults {
            url: url.clone(),
            title: title.clone(),
            description: description.clone(),
            tags: tags.clone(),
            current_tags,
        };

        let config = app_mgr.config()?;
        let rw_lock = config;
        let config = rw_lock.read().unwrap();
        let rules = &config.rules;

        if let Some(u) = url {
            for rule in rules.iter() {
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
                        title: rule_title,
                        description: rule_description,
                        tags: rule_tags,
                    } => {
                        if let Some(title) = rule_title {
                            editor_defaults.title = Some(title.clone());
                        }
                        if let Some(description) = rule_description {
                            editor_defaults.description = Some(description.clone());
                        }
                        if let Some(tags) = rule_tags {
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

        let editor_bmark = crate::editor::edit(editor_defaults)?;

        url = Some(editor_bmark.url);
        if let crate::editor::EditorValue::Set(value) = editor_bmark.title {
            title = Some(value)
        }

        if let crate::editor::EditorValue::Set(value) = editor_bmark.description {
            description = Some(value)
        }

        if let crate::editor::EditorValue::Set(value) = editor_bmark.tags {
            tags = Some(value)
        }
    } else if url.is_none() {
        bail!("url cannot be empty");
    }

    let url = url.unwrap_or_default();

    let bmark_create = BookmarkCreate {
        title,
        description,
        tags: tags.map(parse_tags),
        url,
        ..Default::default()
    };

    let add_opts = crate::app::backend::AddOpts {
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
    println!("{}", serde_json::to_string_pretty(&bmark).unwrap());
    Ok(())
}

pub fn handle_meta(url: String, no_https_upgrade: bool, no_headless: bool) -> Result<()> {
    let fetch_meta_opts = crate::app::backend::FetchMetadataOpts {
        no_https_upgrade,
        meta_opts: MetaOptions { no_headless },
    };

    let meta = crate::app::AppLocal::fetch_metadata(&url, fetch_meta_opts)?;

    if let Some(ref image) = meta.image {
        std::fs::write("screenshot.png", &image).unwrap();
    };

    if let Some(ref icon) = meta.icon {
        std::fs::write("icon.png", &icon).unwrap();
    };

    println!("{}", serde_json::to_string_pretty(&meta).unwrap());
    Ok(())
}

pub fn handle_rule(action: super::types::RulesArgs, config: &mut Config) -> Result<()> {
    match action {
        super::types::RulesArgs::Add {
            url,
            title,
            description,
            tags,
            action,
        } => match action {
            super::types::RuleAction::Update {
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
        super::types::RulesArgs::Delete {} => todo!(),
        super::types::RulesArgs::List {} => {
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
                    println!("  tags: {tags:?}");
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

fn handle_action(
    bmarks: Vec<crate::bookmarks::Bookmark>,
    query: SearchQuery,
    app_mgr: Box<dyn AppBackend>,
    action: Option<ActionArgs>,
) -> Result<()> {
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
            append_tags,
            remove_tags,
        }) => {
            let bmark_update = crate::bookmarks::BookmarkUpdate {
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
                println!("The update did nothing");
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

            let count = app_mgr.search_update(query, bmark_update)?;
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

            let count = app_mgr.search_delete(query)?;
            println!("{} items removed", count);
            Ok(())
        }
    }
}
