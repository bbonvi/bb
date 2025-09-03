use crate::{
    app::backend::AppBackend,
    config::Config,
    cli::commands::{SearchCommand, AddCommand, MetaCommand, RuleCommand, AddOptions, RuleAction, RuleUpdateAction},
};
use anyhow::Result;

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
    // Convert ActionArgs to ActionCommand
    let action_command = action.map(|action| match action {
        ActionArgs::Update { url, title, description, tags, append_tags, remove_tags } => {
            crate::cli::commands::ActionCommand::Update {
                url, title, description, tags, append_tags, remove_tags
            }
        }
        ActionArgs::Delete { yes, force } => {
            crate::cli::commands::ActionCommand::Delete { yes, force }
        }
    });

    let search_command = SearchCommand::new(
        url, title, description, tags, id, exact, count, action_command
    )?;
    
    search_command.execute(app_mgr).map_err(|e| anyhow::anyhow!(e))
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
    let options = AddOptions {
        use_editor,
        no_https_upgrade,
        no_headless,
        no_meta,
        async_meta,
    };

    let add_command = AddCommand::new(url, title, description, tags, options)?;
    add_command.execute(app_mgr).map_err(|e| anyhow::anyhow!(e))
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
