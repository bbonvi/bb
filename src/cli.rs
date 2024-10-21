use clap::{Args as ClapArgs, Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Args {
    #[command(subcommand)]
    pub command: Command,
}

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

#[derive(Subcommand, Debug)]
pub enum Command {
    /// This is for internal use only!
    /// I'm not sure if it still works.
    /// Might delete later.
    #[clap(hide = true)]
    MigrateBuku {},

    /// Generate api docs in markdown format
    #[cfg(feature = "markdown-docs")]
    #[clap(hide = true)]
    MarkdownDocs {},

    /// Start bb as a service.
    Daemon {
        // #[clap(trailing_var_arg=true, value_parser = ["database", "scraper"])]
        // kind: Vec<String>,
    },
    /// Search bookmark
    Search {
        /// a url
        #[clap(short, long)]
        url: Option<String>,

        /// Bookmark title
        #[clap(short, long)]
        title: Option<String>,

        /// Bookmark description
        #[clap(short, long)]
        description: Option<String>,

        /// Bookmark tags
        #[clap(short = 'g', long, allow_hyphen_values = true)]
        tags: Option<String>,

        /// id
        #[clap(short, long)]
        id: Option<u64>,

        /// Exact search. False by default.
        #[clap(short, long, default_value = "false")]
        exact: bool,

        /// Print the count
        #[clap(short = 'c', long, default_value = "false")]
        count: bool,

        #[clap(subcommand)]
        action: Option<ActionArgs>,
    },
    Add {
        #[clap(long, default_value = "false")]
        editor: bool,

        /// a url
        #[clap(allow_hyphen_values = true, hide = true)]
        url: Option<String>,

        /// Bookmark title
        #[clap(short, long)]
        title: Option<String>,

        /// Bookmark description
        #[clap(short, long)]
        description: Option<String>,

        /// Bookmark tags
        #[clap(short = 'g', long)]
        tags: Option<String>,

        /// fetch metadata in background (only when used as client)
        #[clap(long, default_value = "false")]
        async_meta: bool,

        #[clap(flatten)]
        meta_args: MetaArgs,
    },
    /// Query website meta data
    Meta {
        /// A url
        #[clap(allow_hyphen_values = true, hide = true)]
        url: String,

        #[clap(flatten)]
        meta_args: MetaArgs,
    },
    /// Manage automated rules
    Rule {
        #[clap(subcommand)]
        action: RulesArgs,
    },
}
