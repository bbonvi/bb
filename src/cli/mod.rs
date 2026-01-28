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

        /// Keyword search across title, description, url, and tags (use #tag for tag search)
        #[clap(short, long)]
        keyword: Option<String>,

        /// id
        #[clap(short, long)]
        id: Option<u64>,

        /// Exact search. False by default.
        #[clap(short, long, default_value = "false")]
        exact: bool,

        /// Semantic search query (find bookmarks by meaning)
        #[clap(short = 's', long = "sem")]
        semantic: Option<String>,

        /// Similarity threshold for semantic search (0.0-1.0)
        #[clap(long)]
        threshold: Option<f32>,

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
    /// Compress existing images to WebP format
    Compress {
        /// Show what would be done without making changes
        #[clap(long)]
        dry_run: bool,

        /// Skip confirmation prompt
        #[clap(short, long)]
        yes: bool,
    },
    /// Create a backup archive of bb data
    Backup {
        /// Output path for the archive (default: ./bb-backup-{timestamp}.tar.gz)
        path: Option<std::path::PathBuf>,
    },
    /// Import data from a backup archive
    Import {
        /// Path to the backup archive
        path: std::path::PathBuf,

        /// Skip confirmation prompt
        #[clap(short, long)]
        yes: bool,
    },
}
