use clap::{Args as ClapArgs, Parser, Subcommand, ValueEnum};

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

    /// Don't use duckduckgo as a fallback for metadata scrape
    #[clap(long, default_value = "false")]
    pub no_duck: bool,
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

        /// Bookmark tags
        #[clap(short = 'g', long)]
        tags: Option<String>,

        #[clap(flatten)]
        meta_args: MetaArgs,
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
    Update {
        /// Bookmark title
        #[clap(long)]
        title: Option<String>,

        /// Bookmark description
        #[clap(long)]
        description: Option<String>,

        /// Bookmark tags
        // #[clap(long, value_delimiter = ',', num_args = 1..)]
        #[clap(long)]
        tags: Option<String>,
    },
}

#[derive(Subcommand, Debug, Clone)]
pub enum RulesArgs {
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
    Delete {},
    List {},
}

/// Doc comment
#[derive(Subcommand, Debug)]
pub enum Command {
    MigrateBuku {},
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
        /// a url
        #[clap(allow_hyphen_values = true, hide = true)]
        url: String,

        /// Bookmark title
        #[clap(short, long)]
        title: Option<String>,

        /// Bookmark description
        #[clap(short, long)]
        description: Option<String>,

        /// Bookmark tags
        #[clap(short = 'g', long)]
        tags: Option<String>,

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
    /// Query website meta data
    Rule {
        #[clap(subcommand)]
        action: RulesArgs,
    },
}
