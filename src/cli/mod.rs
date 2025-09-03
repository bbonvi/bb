use clap::{Parser, Subcommand};

mod handlers;
mod types;
mod commands;
mod errors;
mod validation;

pub use handlers::*;
pub use types::*;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
pub struct Args {
    #[command(subcommand)]
    pub command: Command,
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

        /// Fuzzy search across title, description, url, and tags (use #tag for tag search)
        #[clap(short, long)]
        fuzzy: Option<String>,

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
