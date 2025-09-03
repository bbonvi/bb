use clap::{Args as ClapArgs, Subcommand};

#[derive(ClapArgs, Debug, Clone)]
pub struct MetaArgs {
    /// Don't try to upgrade to https.
    #[clap(long, default_value = "false")]
    pub no_https_upgrade: bool,

    /// Don't use headless browser to capture
    /// screenshots and metadata
    #[clap(long, default_value = "false")]
    pub no_headless: bool,

    /// Don't fetch meta at all
    #[clap(long, default_value = "false")]
    pub no_meta: bool,
}

#[derive(Subcommand, Debug, Clone)]
pub enum ActionArgs {
    /// Update found bookmarks
    Update {
        /// a url
        #[clap(short, long)]
        url: Option<String>,

        /// Bookmark title
        #[clap(short, long)]
        title: Option<String>,

        /// Bookmark description
        #[clap(short, long)]
        description: Option<String>,

        /// Replace tags
        #[clap(long)]
        tags: Option<String>,

        /// Appends tags
        #[clap(short = 'a', long)]
        append_tags: Option<String>,

        /// Delete tags
        #[clap(short = 'r', long)]
        remove_tags: Option<String>,
    },
    /// Delete found bookmarks
    Delete {
        /// Auto confirm
        #[clap(short, long, default_value = "false")]
        yes: bool,

        /// Don't ask for confirmation when performing dangerous delete.
        /// (e.g. when attempting to delete all bookmarks)
        #[clap(short, long, default_value = "false")]
        force: bool,
    },
}

#[derive(Subcommand, Debug, Clone)]
pub enum RuleAction {
    /// Perform an Update action.
    Update {
        /// Set bookmark title
        #[clap(long)]
        title: Option<String>,

        /// Set bookmark description
        #[clap(long)]
        description: Option<String>,

        /// Add tags
        #[clap(long)]
        tags: Option<String>,
    },
}

#[derive(Subcommand, Debug, Clone)]
pub enum RulesArgs {
    /// Create new rule
    Add {
        /// A regex matching bookmark url
        #[clap(long)]
        url: Option<String>,

        /// A regex matching bookmark title
        #[clap(long)]
        title: Option<String>,

        /// A regex matching bookmark description
        #[clap(long)]
        description: Option<String>,

        /// A list of tags bookmark will be matched by (all tags has to match)
        #[clap(long)]
        tags: Option<String>,

        /// A list of tags bookmark will be matched by (all tags has to match)
        #[clap(subcommand)]
        action: RuleAction,
    },
    /// List all rules.
    List {},
    /// UNIMPLEMENTED! Edit config.yaml directly.
    Delete {},
}
