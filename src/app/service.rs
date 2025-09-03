use crate::{
    app::backend::{AddOpts, AppBackend, RefreshMetadataOpts},
    bookmarks::{BookmarkCreate, BookmarkUpdate, SearchQuery},
    config::Config,
};
use anyhow::Result;
use std::sync::{Arc, RwLock};

pub struct AppService {
    backend: Box<dyn AppBackend>,
}

impl AppService {
    pub fn new(backend: Box<dyn AppBackend>) -> Self {
        Self { backend }
    }

    pub fn into_backend(self) -> Box<dyn AppBackend> {
        self.backend
    }

    #[allow(dead_code)]
    pub fn search_bookmarks(
        &self,
        query: SearchQuery,
        count_only: bool,
    ) -> Result<Vec<crate::bookmarks::Bookmark>> {
        let bookmarks = self.backend.search(query)?;
        
        if count_only {
            println!("{} bookmarks found", bookmarks.len());
            return Ok(vec![]);
        }
        
        Ok(bookmarks)
    }

    #[allow(dead_code)]
    pub fn create_bookmark(
        &self,
        create: BookmarkCreate,
        opts: AddOpts,
    ) -> Result<crate::bookmarks::Bookmark> {
        Ok(self.backend.create(create, opts)?)
    }

    #[allow(dead_code)]
    pub fn update_bookmark(
        &self,
        id: u64,
        update: BookmarkUpdate,
    ) -> Result<crate::bookmarks::Bookmark> {
        Ok(self.backend.update(id, update)?)
    }

    #[allow(dead_code)]
    pub fn delete_bookmark(&self, id: u64) -> Result<()> {
        Ok(self.backend.delete(id).map(|_| ())?)
    }

    #[allow(dead_code)]
    pub fn search_and_update(
        &self,
        query: SearchQuery,
        update: BookmarkUpdate,
    ) -> Result<usize> {
        Ok(self.backend.search_update(query, update)?)
    }

    #[allow(dead_code)]
    pub fn search_and_delete(&self, query: SearchQuery) -> Result<usize> {
        Ok(self.backend.search_delete(query)?)
    }

    #[allow(dead_code)]
    pub fn refresh_metadata(
        &self,
        id: u64,
        opts: RefreshMetadataOpts,
    ) -> Result<()> {
        Ok(self.backend.refresh_metadata(id, opts)?)
    }

    #[allow(dead_code)]
    pub fn get_total_count(&self) -> Result<usize> {
        Ok(self.backend.total()?)
    }

    #[allow(dead_code)]
    pub fn get_tags(&self) -> Result<Vec<String>> {
        Ok(self.backend.tags()?)
    }

    pub fn get_config(&self) -> Result<Arc<RwLock<Config>>> {
        Ok(self.backend.config()?)
    }

    #[allow(dead_code)]
    pub fn update_config(&self, config: Config) -> Result<()> {
        Ok(self.backend.update_config(config)?)
    }

    #[allow(dead_code)]
    pub fn upload_cover(&self, id: u64, file: Vec<u8>) -> Result<crate::bookmarks::Bookmark> {
        Ok(self.backend.upload_cover(id, file)?)
    }

    #[allow(dead_code)]
    pub fn upload_icon(&self, id: u64, file: Vec<u8>) -> Result<crate::bookmarks::Bookmark> {
        Ok(self.backend.upload_icon(id, file)?)
    }
}
