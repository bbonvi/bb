use crate::{
    bookmarks::{
        Bookmark, BookmarkCreate, BookmarkMgrBackend, BookmarkMgrJson, BookmarkUpdate, SearchQuery,
    },
    scrape::{Meta, MetaOptions, MetadataMgr, MetadataMgrBackend},
    storage::{StorageMgrBackend, StorageMgrLocal},
};

pub struct App {
    bookmarks: Box<dyn BookmarkMgrBackend>,
    storage: Box<dyn StorageMgrBackend>,
    metadata: Box<dyn MetadataMgrBackend>,
}

impl App {
    pub fn local() -> Self {
        let bookmarks = Box::new(BookmarkMgrJson::load());
        let storage = Box::new(StorageMgrLocal::new("./uploads"));
        let metadata = Box::new(MetadataMgr::new());

        Self {
            bookmarks,
            storage,
            metadata,
        }
    }
}

impl App {
    pub fn search(&self, query: SearchQuery) -> Vec<Bookmark> {
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

        self.bookmarks.search(query)
    }

    pub fn add(
        &mut self,
        bmark_create: BookmarkCreate,
        no_https_upgrade: bool,
        no_headless: bool,
        no_meta: bool,
    ) -> Option<Bookmark> {
        let query = SearchQuery {
            url: Some(bmark_create.url.clone()),
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
            self.fetch_metadata(&bmark_create.url, no_https_upgrade, no_headless)
        } else {
            None
        };

        let mut bmark_create = bmark_create;

        if let Some(ref meta) = meta {
            if bmark_create.title.is_none() {
                bmark_create.title = meta.title.clone();
            }
            if bmark_create.description.is_none() {
                bmark_create.description = meta.description.clone();
            }
        }

        let bookmarks = self.bookmarks.add(bmark_create).first().cloned();

        // save images
        if let Some(ref bookmark) = bookmarks {
            match meta {
                Some(ref meta) => {
                    // let mut bmark_create = BookmarkUpdate {
                    //     has_image: true,
                    //     has_icon: true,
                    //     ..Default::default()
                    // };
                    //
                    // if let Some(ref image) = meta.image {
                    //     self.storage.write(&bookmark.id.to_string(), &image);
                    //     bmark_create.has_image = true;
                    // };
                    //
                    // if let Some(ref icon) = meta.icon {
                    //     self.storage.write(&bookmark.id.to_string(), &icon);
                    //
                    //     bmark_create.has_icon = true;
                    // };
                    //
                    // let bookmarks = self
                    //     .bookmarks
                    //     .update(bookmark.id, bmark_create)
                    //     .first()
                    //     .cloned();
                }
                _ => {}
            }
        }

        bookmarks
    }

    pub fn update(&mut self, id: u64, bmark_update: BookmarkUpdate) -> Option<Bookmark> {
        self.bookmarks.update(id, bmark_update)
    }

    pub fn delete(&mut self, id: u64) -> Option<bool> {
        self.bookmarks.delete(id)
    }

    pub fn fetch_metadata(
        &self,
        url: &str,
        no_https_upgrade: bool,
        no_headless: bool,
    ) -> Option<Meta> {
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
            .retrieve(&url_parsed.to_string(), opts.clone());

        if meta.is_none() && tried_https {
            println!("https attempt failed. trying http.");
            url_parsed.set_scheme("http").unwrap();
            meta = self.metadata.retrieve(&url_parsed.to_string(), opts);
        }

        meta
    }

    pub fn schedule_fetch_and_update_metadata(
        &self,
        id: u64,
        no_https_upgrade: bool,
        no_headless: bool,
    ) {
        todo!()
    }
}
