use crate::{
    app::service::AppService,
    bookmarks::{BookmarkManager, SearchQuery},
    config::Config,
    storage::StorageManager,
    cli::commands::{SearchCommand, AddCommand, MetaCommand, RuleCommand, CompressCommand, AddOptions, RuleAction, RuleUpdateAction, SearchCommandParams},
};
use anyhow::Result;
use std::sync::Arc;

use super::types::ActionArgs;

/// Parameters for search operations
#[derive(Debug)]
pub struct SearchParams {
    pub url: Option<String>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub tags: Option<String>,
    pub keyword: Option<String>,
    pub id: Option<u64>,
    pub exact: bool,
    pub semantic: Option<String>,
    pub threshold: Option<f32>,
    pub count: bool,
    pub action: Option<ActionArgs>,
}

/// Parameters for add operations
#[derive(Debug)]
pub struct AddParams {
    pub use_editor: bool,
    pub url: Option<String>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub tags: Option<String>,
    pub no_https_upgrade: bool,
    pub no_headless: bool,
    pub no_meta: bool,
    pub async_meta: bool,
}

pub fn handle_search(params: SearchParams, app_service: AppService) -> Result<()> {
    // Convert ActionArgs to ActionCommand
    let action_command = params.action.map(|action| match action {
        ActionArgs::Update { url, title, description, tags, append_tags, remove_tags } => {
            crate::cli::commands::ActionCommand::Update {
                url, title, description, tags, append_tags, remove_tags
            }
        }
        ActionArgs::Delete { yes, force } => {
            crate::cli::commands::ActionCommand::Delete { yes, force }
        }
    });

    let search_command = SearchCommand::new(SearchCommandParams {
        url: params.url,
        title: params.title,
        description: params.description,
        tags: params.tags,
        keyword: params.keyword,
        id: params.id,
        exact: params.exact,
        semantic: params.semantic,
        threshold: params.threshold,
        count: params.count,
        action: action_command,
    })?;
    
    search_command.execute(app_service).map_err(|e| anyhow::anyhow!(e))
}

pub fn handle_add(params: AddParams, app_service: AppService) -> Result<()> {
    let options = AddOptions {
        use_editor: params.use_editor,
        no_https_upgrade: params.no_https_upgrade,
        no_headless: params.no_headless,
        no_meta: params.no_meta,
        async_meta: params.async_meta,
    };

    let add_command = AddCommand::new(params.url, params.title, params.description, params.tags, options)?;
    add_command.execute(app_service).map_err(|e| anyhow::anyhow!(e))
}

pub fn handle_meta(url: String, _no_https_upgrade: bool, no_headless: bool) -> Result<()> {
    let meta_opts = crate::metadata::MetaOptions { no_headless };
    let meta_command = MetaCommand::new(url, meta_opts)?;
    meta_command.execute().map_err(|e| anyhow::anyhow!(e))
}

pub fn handle_rule(action: super::types::RulesArgs, config: &mut Config) -> Result<()> {
    let rule_action = match action {
        super::types::RulesArgs::Add {
            url,
            title,
            description,
            tags,
            action,
        } => {
            let update_action = match action {
                super::types::RuleAction::Update {
                    title: update_title,
                    description: update_description,
                    tags: update_tags,
                } => RuleUpdateAction {
                    title: update_title,
                    description: update_description,
                    tags: update_tags,
                }
            };
            
            RuleAction::Add {
                url,
                title,
                description,
                tags,
                update_action,
            }
        }
        super::types::RulesArgs::Delete {} => RuleAction::Delete,
        super::types::RulesArgs::List {} => RuleAction::List,
    };

    let rule_command = RuleCommand::new(rule_action)?;
    rule_command.execute(config).map_err(|e| anyhow::anyhow!(e))
}

pub fn handle_compress<S: StorageManager>(
    dry_run: bool,
    yes: bool,
    storage: &S,
    bmark_mgr: Arc<dyn BookmarkManager>,
    config: &Config,
) -> Result<()> {
    // Get all bookmarks
    let bookmarks = bmark_mgr.search(SearchQuery::default())?;

    let img_config = &config.images;

    let cmd = CompressCommand::new(dry_run, yes);
    cmd.execute(
        storage,
        &bookmarks,
        img_config.max_size,
        img_config.quality,
        |id, update| bmark_mgr.update(id, update),
    ).map_err(|e| anyhow::anyhow!(e))
}
