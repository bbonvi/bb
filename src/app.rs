use std::sync::{Arc, RwLock};

use crate::{
    bookmarks::{Bookmark, BookmarkBackend, BookmarkShallow, Bookmarks, Query},
    scrape::{Meta, MetaBackend, MetaLocalService, MetaOptions},
    storage::{LocalStorage, StorageBackend},
};

#[derive(Clone)]
pub struct App {
    bookmarks: Arc<RwLock<dyn BookmarkBackend + Send + Sync>>,
    storage: Arc<RwLock<dyn StorageBackend + Send + Sync>>,
    metadata: Arc<RwLock<dyn MetaBackend + Send + Sync>>,
}

impl App {
    pub fn local() -> Self {
        let bookmarks = Arc::new(RwLock::new(Bookmarks::load()));
        let storage = Arc::new(RwLock::new(LocalStorage::new("./uploads")));
        let metadata = Arc::new(RwLock::new(MetaLocalService::new()));

        Self {
            bookmarks,
            storage,
            metadata,
        }
    }
}

impl App {
    pub fn search(&self, query: Query) -> Vec<Bookmark> {
        let mut query = query;
        // prevent query against empty strings
        {
            if query.title.clone().unwrap_or_default() == "" {
                query.title = None;
            };
            if query.description.clone().unwrap_or_default() == "" {
                query.description = None;
            };
            if query.url.clone().unwrap_or_default() == "" {
                query.url = None;
            };
        }

        self.bookmarks.read().unwrap().search(query)
    }

    pub fn add(
        &self,
        shallow_bookmark: BookmarkShallow,
        no_https_upgrade: bool,
        no_headless: bool,
        no_meta: bool,
    ) -> Option<Bookmark> {
        let query = Query {
            url: Some(shallow_bookmark.url.clone()),
            ..Default::default()
        };

        if let Some(b) = self.search(query).first() {
            eprintln!(
                "bookmark with following url already exists at index {0}",
                b.id
            );

            return None;
        }

        let meta = if !no_meta {
            match self.fetch_meta(&shallow_bookmark.url, no_https_upgrade, no_headless) {
                Some(meta) => {
                    if let Some(ref image) = meta.image {
                        std::fs::write("screenshot.png", &image).unwrap();
                    };
                    if let Some(ref icon) = meta.icon {
                        std::fs::write("icon.png", &icon).unwrap();
                    };
                    Some(meta)
                }
                None => None,
            }
        } else {
            None
        };

        let mut shallow_bookmark = shallow_bookmark;

        if let Some(meta) = meta {
            if shallow_bookmark.title.is_none() {
                shallow_bookmark.title = meta.title;
            }
            if shallow_bookmark.description.is_none() {
                shallow_bookmark.description = meta.description;
            }
        }

        let bookmark = self.bookmarks.write().unwrap().add(shallow_bookmark);

        bookmark.first().cloned()
    }

    pub fn update(&self, id: u64, shallow_bookmark: BookmarkShallow) -> Option<Bookmark> {
        self.bookmarks.write().unwrap().update(id, shallow_bookmark)
    }

    pub fn delete(&self, id: u64) -> Option<bool> {
        self.bookmarks.write().unwrap().delete(id)
    }

    pub fn fetch_meta(&self, url: &str, no_https_upgrade: bool, no_headless: bool) -> Option<Meta> {
        let mut url_parsed = reqwest::Url::parse(&url).unwrap();
        let mut tried_https = false;
        if url_parsed.scheme() == "http" && !no_https_upgrade {
            println!("http url provided. trying https first");
            url_parsed.set_scheme("https").unwrap();
            tried_https = true;
        }

        let opts = MetaOptions { no_headless };

        let mut meta = self
            .metadata
            .read()
            .unwrap()
            .retrieve(&url_parsed.to_string(), opts.clone());

        if meta.is_none() && tried_https {
            println!("https attempt failed. trying http.");
            url_parsed.set_scheme("http").unwrap();
            meta = self
                .metadata
                .read()
                .unwrap()
                .retrieve(&url_parsed.to_string(), opts);
        }

        meta
    }

    pub fn schedule_fetch_meta_update(&self, id: u64, no_https_upgrade: bool, no_headless: bool) {
        todo!()
    }
}
