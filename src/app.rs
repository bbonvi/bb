use std::{
    collections::VecDeque,
    sync::{Arc, RwLock},
    thread::sleep,
    time::Duration,
};

use anyhow::{bail, Context};

use crate::{
    bookmarks::{
        Bookmark, BookmarkCreate, BookmarkMgrBackend, BookmarkMgrJson, BookmarkUpdate, SearchQuery,
    },
    eid::Eid,
    metadata::{fetch_meta, MetaOptions, Metadata},
    storage::{StorageMgrBackend, StorageMgrLocal},
};

fn guess_filetype(url: &str) -> Option<String> {
    if url.contains(".jpg") || url.contains(".jpeg") {
        return Some(String::from("jpg"));
    }

    if url.contains(".webp") {
        return Some(String::from("webp"));
    }

    if url.contains(".png") {
        return Some(String::from("png"));
    }

    if url.contains(".svg") {
        return Some(String::from("svg"));
    }

    if url.contains(".bmp") {
        return Some(String::from("bmp"));
    }

    if url.contains(".gif") {
        return Some(String::from("gif"));
    }

    None
}

pub struct App {
    pub bmark_mgr: Arc<dyn BookmarkMgrBackend>,
    pub storage_mgr: Arc<dyn StorageMgrBackend>,

    pub metadata_queue: Arc<RwLock<VecDeque<Option<(u64, String, FetchMetadataOpts)>>>>,
    pub queue_handle: Option<std::thread::JoinHandle<()>>,
}

#[derive(Debug, Clone, Default)]
pub struct AddOpts {
    pub no_https_upgrade: bool,
    pub async_meta: bool,
    pub meta_opts: Option<MetaOptions>,
}

#[derive(Debug, Clone, Default)]
pub struct FetchMetadataOpts {
    pub no_https_upgrade: bool,
    pub meta_opts: MetaOptions,
}

impl App {
    pub fn local() -> Self {
        let bmark_mgr = Arc::new(BookmarkMgrJson::load());
        let storage_mgr = Arc::new(StorageMgrLocal::new("./uploads"));
        let metadata_queue = Arc::new(RwLock::new(VecDeque::new()));

        let handle = std::thread::spawn({
            let bmark_mgr = bmark_mgr.clone();
            let storage_mgr = storage_mgr.clone();
            let metadata_queue = metadata_queue.clone();

            move || {
                Self::start_queue(metadata_queue, bmark_mgr, storage_mgr);
            }
        });

        Self {
            bmark_mgr,
            storage_mgr,
            metadata_queue,
            queue_handle: Some(handle),
        }
    }

    pub fn start_queue(
        queue: Arc<RwLock<VecDeque<Option<(u64, String, FetchMetadataOpts)>>>>,
        bookmark_mgr: Arc<dyn BookmarkMgrBackend>,
        storage_mgr: Arc<dyn StorageMgrBackend>,
    ) {
        loop {
            if queue.read().unwrap().is_empty() {
                sleep(Duration::from_millis(200));
            }

            match queue.write().unwrap().pop_back() {
                Some(None) => break,
                None => {}
                Some(Some((id, url, opts))) => {
                    println!("picked up a job...");
                    let meta = match Self::fetch_metadata(&url, opts) {
                        Ok(meta) => meta,
                        Err(err) => {
                            eprintln!("{err}");
                            continue;
                        }
                    };

                    let bookmarks = match bookmark_mgr.search(SearchQuery {
                        id: Some(id),
                        ..Default::default()
                    }) {
                        Ok(b) => b,
                        Err(err) => {
                            eprintln!("{err}");
                            continue;
                        }
                    };

                    let bookmark = match bookmarks.first() {
                        Some(b) => b,
                        None => {
                            eprintln!("bookmark {id} not found");
                            continue;
                        }
                    };

                    if let Err(err) = Self::merge_metadata(
                        bookmark.clone(),
                        meta,
                        storage_mgr.clone(),
                        bookmark_mgr.clone(),
                    ) {
                        eprintln!("{err}");
                    }
                }
            }
        }
    }

    pub fn search(&self, query: SearchQuery) -> anyhow::Result<Vec<Bookmark>> {
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

        self.bmark_mgr.search(query)
    }

    pub fn add(&self, bmark_create: BookmarkCreate, opts: AddOpts) -> anyhow::Result<Bookmark> {
        let url = bmark_create.url.clone();
        let bookmark = self.bmark_mgr.add(bmark_create)?;

        if let Some(meta_opts) = opts.meta_opts {
            if opts.async_meta {
                self.schedule_fetch_and_update_metadata(
                    &bookmark,
                    FetchMetadataOpts {
                        no_https_upgrade: opts.no_https_upgrade,
                        meta_opts: meta_opts.clone(),
                    },
                );
            } else {
                let meta = Self::fetch_metadata(
                    &url,
                    FetchMetadataOpts {
                        no_https_upgrade: opts.no_https_upgrade,
                        meta_opts,
                    },
                )?;

                return Self::merge_metadata(
                    bookmark.clone(),
                    meta,
                    self.storage_mgr.clone(),
                    self.bmark_mgr.clone(),
                )?
                .context("bookmark not found");
            }
        }

        Ok(bookmark)
    }

    pub fn update(
        &mut self,
        id: u64,
        bmark_update: BookmarkUpdate,
    ) -> anyhow::Result<Option<Bookmark>> {
        self.bmark_mgr.update(id, bmark_update)
    }

    pub fn delete(&mut self, id: u64) -> anyhow::Result<Option<bool>> {
        self.bmark_mgr.delete(id)
    }

    pub fn fetch_metadata(url: &str, opts: FetchMetadataOpts) -> anyhow::Result<Metadata> {
        let mut url_parsed = reqwest::Url::parse(&url).unwrap();
        let mut tried_https = false;
        if url_parsed.scheme() == "http" && !opts.no_https_upgrade {
            println!("http url provided. trying https first");
            url_parsed.set_scheme("https").unwrap();
            tried_https = true;
        }

        let err = match fetch_meta(&url_parsed.to_string(), opts.meta_opts.clone()) {
            Ok(m) => return Ok(m),
            Err(err) => Err(err),
        };

        if tried_https {
            println!("https attempt failed. trying http.");
            url_parsed.set_scheme("http").unwrap();
            return fetch_meta(&url_parsed.to_string(), opts.meta_opts.clone());
        }

        return err;
    }

    pub fn schedule_fetch_and_update_metadata(
        &self,
        bookmark: &Bookmark,
        meta_opts: FetchMetadataOpts,
    ) {
        self.metadata_queue.write().unwrap().push_front(Some((
            bookmark.id,
            bookmark.url.clone(),
            meta_opts,
        )));
    }

    fn merge_metadata(
        bookmark: Bookmark,
        meta: Metadata,
        storage_mgr: Arc<dyn StorageMgrBackend>,
        bmark_mgr: Arc<dyn BookmarkMgrBackend>,
    ) -> anyhow::Result<Option<Bookmark>> {
        let mut bmark_update = BookmarkUpdate {
            ..Default::default()
        };
        if bookmark.title.is_empty() {
            bmark_update.title = meta.title;
        }

        if bookmark.description.is_empty() {
            bmark_update.description = meta.description;
        }

        if let Some(ref image) = meta.image {
            let filetype = meta
                .image_url
                .as_ref()
                .map(|url| guess_filetype(&url).unwrap_or("png".to_string()))
                .unwrap_or("png".to_string());

            let image_id = format!("{}.{}", Eid::new(), filetype);

            storage_mgr.write(&image_id, &image);
            bmark_update.image_id = Some(image_id.to_string());
        };

        if let Some(ref icon) = meta.icon {
            let filetype = meta
                .image_url
                .as_ref()
                .map(|url| guess_filetype(&url).unwrap_or("png".to_string()))
                .unwrap_or("png".to_string());

            let icon_id = format!("{}.{}", Eid::new(), filetype);

            storage_mgr.write(&icon_id, &icon);
            bmark_update.icon_id = Some(icon_id.to_string());
        };

        bmark_mgr.update(bookmark.id, bmark_update)
    }
}
