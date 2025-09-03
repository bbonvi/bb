use serde::{Deserialize, Serialize};
use std::sync::{Arc, RwLock};

use crate::{bookmarks, config::Config, metadata::MetaOptions};

use super::errors::AppError;

pub trait AppBackend: Send + Sync {
    fn create(
        &self,
        bmark_create: bookmarks::BookmarkCreate,
        opts: AddOpts,
    ) -> anyhow::Result<bookmarks::Bookmark, AppError>;

    fn refresh_metadata(&self, id: u64, opts: RefreshMetadataOpts) -> anyhow::Result<(), AppError>;

    fn update(
        &self,
        id: u64,
        bmark_update: bookmarks::BookmarkUpdate,
    ) -> anyhow::Result<bookmarks::Bookmark, AppError>;
    fn delete(&self, id: u64) -> anyhow::Result<(), AppError>;
    fn search_delete(&self, query: bookmarks::SearchQuery) -> anyhow::Result<usize, AppError>;
    fn search_update(
        &self,
        query: bookmarks::SearchQuery,
        bmark_update: bookmarks::BookmarkUpdate,
    ) -> anyhow::Result<usize, AppError>;
    fn total(&self) -> anyhow::Result<usize, AppError>;
    fn tags(&self) -> anyhow::Result<Vec<String>, AppError>;
    fn search(
        &self,
        query: bookmarks::SearchQuery,
    ) -> anyhow::Result<Vec<bookmarks::Bookmark>, AppError>;

    fn config(&self) -> anyhow::Result<Arc<RwLock<Config>>, AppError>;
    #[allow(dead_code)]
    fn update_config(&self, config: Config) -> anyhow::Result<(), AppError>;
}

#[derive(Debug, Clone, Default)]
pub struct AddOpts {
    pub no_https_upgrade: bool,
    pub async_meta: bool,
    pub meta_opts: Option<MetaOptions>,
    pub skip_rules: bool,
}

#[derive(Debug, Clone, Default)]
pub struct RefreshMetadataOpts {
    pub async_meta: bool,
    pub meta_opts: MetaOptions,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FetchMetadataOpts {
    pub no_https_upgrade: bool,
    pub meta_opts: MetaOptions,
}
