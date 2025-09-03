use crate::{
    app::service::AppService,
    bookmarks::{BookmarkCreate, BookmarkUpdate, SearchQuery},
    metadata::MetaOptions,
    parse_tags,
    cli::{errors::CliResult, validation::*},
};

/// Command for searching bookmarks
#[derive(Debug, Clone)]
pub struct SearchCommand {
    pub query: SearchQuery,
    pub count_only: bool,
    pub action: Option<ActionCommand>,
}

/// Parameters for creating a search command
#[derive(Debug, Clone)]
pub struct SearchCommandParams {
    pub url: Option<String>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub tags: Option<String>,
    pub fuzzy: Option<String>,
    pub id: Option<u64>,
    pub exact: bool,
    pub count: bool,
    pub action: Option<ActionCommand>,
}

impl SearchCommand {
    pub fn new(params: SearchCommandParams) -> CliResult<Self> {
        // Validate search query input
        validate_search_query(&params.url, &params.title, &params.description, &params.tags)?;

        let query = SearchQuery {
            id: params.id,
            title: params.title,
            url: params.url,
            description: params.description,
            tags: params.tags.map(parse_tags),
            fuzzy: params.fuzzy,
            exact: params.exact,
            limit: None,
        };

        Ok(Self {
            query,
            count_only: params.count,
            action: params.action,
        })
    }

    pub fn execute(self, app_service: AppService) -> CliResult<()> {
        let bmarks = app_service.search_bookmarks(self.query.clone(), self.count_only)
            .map_err(|e| crate::cli::errors::CliError::database(e.to_string()))?;

        if bmarks.is_empty() {
            if self.action.is_some() {
                println!("0 items updated");
                return Ok(());
            }
            println!("{}", serde_json::to_string_pretty(&bmarks)
                .map_err(|e| crate::cli::errors::CliError::invalid_input(e.to_string()))?);
            return Ok(());
        }

        if self.count_only {
            println!("{} bookmarks found", bmarks.len());
            return Ok(());
        }

        if let Some(action) = self.action {
            action.execute(bmarks, self.query, app_service)
        } else {
            println!("{}", serde_json::to_string_pretty(&bmarks)
                .map_err(|e| crate::cli::errors::CliError::invalid_input(e.to_string()))?);
            Ok(())
        }
    }
}

/// Command for adding bookmarks
#[derive(Debug, Clone)]
pub struct AddCommand {
    pub url: Option<String>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub tags: Option<String>,
    pub options: AddOptions,
}

#[derive(Debug, Clone)]
pub struct AddOptions {
    pub use_editor: bool,
    pub no_https_upgrade: bool,
    pub no_headless: bool,
    pub no_meta: bool,
    pub async_meta: bool,
}

impl AddCommand {
    pub fn new(
        url: Option<String>,
        title: Option<String>,
        description: Option<String>,
        tags: Option<String>,
        options: AddOptions,
    ) -> CliResult<Self> {
        // Validate bookmark creation input
        validate_bookmark_create(&url, &title, &description, &tags)?;

        Ok(Self {
            url,
            title,
            description,
            tags,
            options,
        })
    }

    pub fn execute(self, app_service: AppService) -> CliResult<()> {
        let mut url = self.url;
        let mut title = self.title;
        let mut description = self.description;
        let mut tags = self.tags;

        if self.options.use_editor {
            let mut current_tags = app_service.get_tags()
                .map_err(|e| crate::cli::errors::CliError::database(e.to_string()))?;
            current_tags.sort();

            let mut editor_defaults = crate::editor::EditorDefaults {
                url: url.clone(),
                title: title.clone(),
                description: description.clone(),
                tags: tags.clone(),
                current_tags,
            };

            let config = app_service.get_config()
                .map_err(|e| crate::cli::errors::CliError::configuration(e.to_string()))?;
            let rules = &config.read().unwrap().rules;

            if let Some(u) = url {
                for rule in rules.iter() {
                    let record = crate::rules::Record {
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
                                let mut curr_tags = editor_defaults.tags
                                    .map(parse_tags)
                                    .unwrap_or_default();
                                curr_tags.append(&mut tags.clone());
                                editor_defaults.tags = Some(curr_tags.join(" "));
                            }
                        }
                    }
                }
            }

            let editor_bmark = crate::editor::edit(editor_defaults)
                .map_err(|e| crate::cli::errors::CliError::invalid_input(e.to_string()))?;

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
            return Err(crate::cli::errors::CliError::validation("url", "URL cannot be empty"));
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
            no_https_upgrade: self.options.no_https_upgrade,
            async_meta: self.options.async_meta,
            meta_opts: if self.options.no_meta {
                None
            } else {
                Some(MetaOptions { 
                    no_headless: self.options.no_headless 
                })
            },
            skip_rules: false,
        };

        let bmark = app_service.create_bookmark(bmark_create, add_opts)
            .map_err(|e| crate::cli::errors::CliError::database(e.to_string()))?;
        
        println!("{}", serde_json::to_string_pretty(&bmark)
            .map_err(|e| crate::cli::errors::CliError::invalid_input(e.to_string()))?);
        Ok(())
    }
}

/// Command for metadata operations
#[derive(Debug, Clone)]
pub struct MetaCommand {
    pub url: String,
    pub options: MetaOptions,
}

impl MetaCommand {
    pub fn new(url: String, options: MetaOptions) -> CliResult<Self> {
        // Validate URL
        validate_url(&url)?;
        
        Ok(Self { url, options })
    }

    pub fn execute(self) -> CliResult<()> {
        let meta = crate::metadata::fetch_meta(&self.url, self.options)
            .map_err(|e| crate::cli::errors::CliError::metadata(e.to_string()))?;

        if let Some(ref image) = meta.image {
            std::fs::write("screenshot.png", image)
                .map_err(|e| crate::cli::errors::CliError::storage(e.to_string()))?;
        }

        if let Some(ref icon) = meta.icon {
            std::fs::write("icon.png", icon)
                .map_err(|e| crate::cli::errors::CliError::storage(e.to_string()))?;
        }

        println!("{}", serde_json::to_string_pretty(&meta)
            .map_err(|e| crate::cli::errors::CliError::invalid_input(e.to_string()))?);
        Ok(())
    }
}

/// Command for rule operations
#[derive(Debug, Clone)]
pub struct RuleCommand {
    pub action: RuleAction,
}

#[derive(Debug, Clone)]
pub enum RuleAction {
    Add {
        url: Option<String>,
        title: Option<String>,
        description: Option<String>,
        tags: Option<String>,
        update_action: RuleUpdateAction,
    },
    Delete,
    List,
}

#[derive(Debug, Clone)]
pub struct RuleUpdateAction {
    pub title: Option<String>,
    pub description: Option<String>,
    pub tags: Option<String>,
}

impl RuleCommand {
    pub fn new(action: RuleAction) -> CliResult<Self> {
        // Validate rule input if it's an Add action
        if let RuleAction::Add { url, title, description, tags, .. } = &action {
            validate_rule_input(url, title, description, tags)?;
        }
        
        Ok(Self { action })
    }

    pub fn execute(self, config: &mut crate::config::Config) -> CliResult<()> {
        match self.action {
            RuleAction::Add {
                url,
                title,
                description,
                tags,
                update_action,
            } => {
                let rule = crate::rules::Rule {
                    url: url.map(|u| u.to_lowercase()),
                    description: description.map(|d| d.to_lowercase()),
                    title: title.map(|d| d.to_lowercase()),
                    tags: tags.clone().map(parse_tags),
                    action: crate::rules::Action::UpdateBookmark {
                        title: update_action.title.map(|u| u.to_lowercase()),
                        description: update_action.description.map(|d| d.to_lowercase()),
                        tags: update_action.tags.clone().map(parse_tags),
                    },
                    comment: None,
                };
                config.rules.insert(0, rule);
                config.save();
            }
            RuleAction::Delete => {
                return Err(crate::cli::errors::CliError::not_supported("Delete rule"));
            }
            RuleAction::List => {
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
                        crate::rules::Action::UpdateBookmark {
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
                    println!();
                }
            }
        }
        Ok(())
    }
}

/// Command for actions on search results
#[derive(Debug, Clone)]
pub enum ActionCommand {
    Update {
        url: Option<String>,
        title: Option<String>,
        description: Option<String>,
        tags: Option<String>,
        append_tags: Option<String>,
        remove_tags: Option<String>,
    },
    Delete {
        yes: bool,
        force: bool,
    },
}

impl ActionCommand {
    pub fn execute(
        self,
        bmarks: Vec<crate::bookmarks::Bookmark>,
        query: SearchQuery,
        app_service: AppService,
    ) -> CliResult<()> {
        match self {
            ActionCommand::Update {
                url,
                title,
                description,
                tags,
                append_tags,
                remove_tags,
            } => {
                // Validate update input
                validate_bookmark_create(&url, &title, &description, &tags)?;
                if let Some(ref append_tags) = append_tags {
                    validate_tags(append_tags)?;
                }
                if let Some(ref remove_tags) = remove_tags {
                    validate_tags(remove_tags)?;
                }

                let bmark_update = BookmarkUpdate {
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
                        inquire::error::InquireResult::Ok(true) => {}
                        inquire::error::InquireResult::Ok(false) => return Ok(()),
                        inquire::error::InquireResult::Err(err) => return Err(crate::cli::errors::CliError::invalid_input(err.to_string())),
                    }
                }

                let count = app_service.search_and_update(query, bmark_update)
                    .map_err(|e| crate::cli::errors::CliError::database(e.to_string()))?;
                println!("{} items updated", count);
                Ok(())
            }
            ActionCommand::Delete { yes, force } => {
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
                        inquire::error::InquireResult::Ok(true) => {}
                        inquire::error::InquireResult::Ok(false) => return Ok(()),
                        inquire::error::InquireResult::Err(err) => return Err(crate::cli::errors::CliError::invalid_input(err.to_string())),
                    }
                }

                if is_wipe {
                    match inquire::prompt_confirmation(
                        "You are about to wipe your entire database. Are you really sure?",
                    ) {
                        inquire::error::InquireResult::Ok(true) => {}
                        inquire::error::InquireResult::Ok(false) => return Ok(()),
                        inquire::error::InquireResult::Err(err) => return Err(crate::cli::errors::CliError::invalid_input(err.to_string())),
                    }
                }

                let count = app_service.search_and_delete(query)
                    .map_err(|e| crate::cli::errors::CliError::database(e.to_string()))?;
                println!("{} items removed", count);
                Ok(())
            }
        }
    }
}
