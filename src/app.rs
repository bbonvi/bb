use std::{
    collections::{HashSet, VecDeque},
    sync::{atomic::AtomicU16, Arc, RwLock},
    thread::sleep,
    time::Duration,
};

use anyhow::{anyhow, bail, Context};

use crate::{
    bookmarks::{
        Bookmark, BookmarkCreate, BookmarkMgrBackend, BookmarkMgrJson, BookmarkUpdate, SearchQuery,
    },
    config::Config,
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
    pub config: Arc<RwLock<Config>>,
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
        let config = Arc::new(RwLock::new(Config::load()));

        let handle = std::thread::spawn({
            let bmark_mgr = bmark_mgr.clone();
            let storage_mgr = storage_mgr.clone();
            let metadata_queue = metadata_queue.clone();

            let config = config.clone();
            move || {
                Self::start_queue(metadata_queue, bmark_mgr, storage_mgr, config);
            }
        });

        Self {
            bmark_mgr,
            storage_mgr,
            metadata_queue,
            queue_handle: Some(handle),
            config,
        }
    }

    pub fn start_queue(
        queue: Arc<RwLock<VecDeque<Option<(u64, String, FetchMetadataOpts)>>>>,
        bookmark_mgr: Arc<dyn BookmarkMgrBackend>,
        storage_mgr: Arc<dyn StorageMgrBackend>,
        config: Arc<RwLock<Config>>,
    ) {
        use std::sync::atomic::Ordering;

        let counter = Arc::new(AtomicU16::new(0));

        loop {
            if queue.read().unwrap().is_empty() {
                sleep(Duration::from_millis(200));
            }

            while counter.load(Ordering::SeqCst) >= 6 {
                sleep(Duration::from_millis(200));
            }

            let mut queue = queue.write().unwrap();

            let job = queue.pop_back();

            drop(queue);

            match job {
                Some(None) => break,
                None => {}
                Some(Some((id, url, opts))) => {
                    let storage_mgr = storage_mgr.clone();
                    let bookmark_mgr = bookmark_mgr.clone();

                    let counter = counter.clone();

                    let config = config.clone();
                    std::thread::spawn(move || {
                        let counter = counter.clone();

                        counter.fetch_add(1, Ordering::SeqCst);

                        let handle_metadata = || {
                            println!("picked up a job...");
                            let meta = Self::fetch_metadata(&url, opts)?;

                            let bookmarks = bookmark_mgr.search(SearchQuery {
                                id: Some(id),
                                ..Default::default()
                            })?;

                            let bookmark = bookmarks
                                .first()
                                .ok_or_else(|| anyhow!("bookmark {id} not found"))?;

                            let bookmark = Self::merge_metadata(
                                bookmark.clone(),
                                meta,
                                storage_mgr.clone(),
                                bookmark_mgr.clone(),
                            )?
                            .context("bookmark {id} not found")?;

                            Ok(bookmark) as anyhow::Result<Bookmark>
                        };

                        let _ = handle_metadata();
                        let _ = Self::apply_rules(id, bookmark_mgr.clone(), config.clone())
                            .map_err(|err| eprintln!("{err}"));

                        counter.fetch_sub(1, Ordering::SeqCst);
                    });
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

    fn apply_rules(
        id: u64,
        bmark_mgr: Arc<dyn BookmarkMgrBackend>,
        config: Arc<RwLock<Config>>,
    ) -> anyhow::Result<Option<Bookmark>> {
        let query = SearchQuery {
            id: Some(id),
            ..Default::default()
        };

        let bmark = bmark_mgr
            .search(query)
            .map(|b| b.first().cloned())?
            .ok_or_else(|| anyhow!("bookmark not found"))?;

        let config = config.read().unwrap();

        let mut bmark_update = BookmarkUpdate {
            title: if bmark.title.is_empty() {
                None
            } else {
                Some(bmark.title.clone())
            },
            description: if bmark.description.is_empty() {
                None
            } else {
                Some(bmark.description.clone())
            },
            url: Some(bmark.url.clone()),
            tags: if bmark.tags.is_empty() {
                None
            } else {
                Some(bmark.tags.clone())
            },
            ..Default::default()
        };

        // for rule in config.rules.iter().filter(|r| r.is_match(&query)) {
        for rule in config.rules.iter() {
            // recreating query because it could've been changed by previous rule
            let query = SearchQuery {
                url: bmark_update.url.clone(),
                title: bmark_update.title.clone(),
                description: bmark_update.description.clone(),
                tags: bmark_update.tags.clone(),
                ..Default::default()
            };
            if !rule.is_match(&query) {
                continue;
            }

            match &rule.action {
                crate::rules::Action::UpdateBookmark {
                    title,
                    description,
                    tags,
                } => {
                    if title.is_some() {
                        bmark_update.title = title.clone();
                    }
                    if description.is_some() {
                        bmark_update.description = description.clone();
                    }
                    if let Some(tags) = tags {
                        let mut curr_tags = bmark_update.tags.take().unwrap_or_default();
                        curr_tags.append(&mut tags.clone());
                        bmark_update.tags = Some(curr_tags);
                    }
                }
            }
        }

        bmark_mgr.update(bmark.id, bmark_update)
    }

    pub fn add(&self, bmark_create: BookmarkCreate, opts: AddOpts) -> anyhow::Result<Bookmark> {
        let url = bmark_create.url.clone();

        // create empty bookmark
        let bookmark = self.bmark_mgr.add(bmark_create)?;

        // add metadata
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
                // attempt to fetch and merge metadata
                let with_meta = {
                    let meta = Self::fetch_metadata(
                        &url,
                        FetchMetadataOpts {
                            no_https_upgrade: opts.no_https_upgrade,
                            meta_opts,
                        },
                    )?;

                    let bookmark = Self::merge_metadata(
                        bookmark.clone(),
                        meta,
                        self.storage_mgr.clone(),
                        self.bmark_mgr.clone(),
                    )?
                    .context("bookmark not found")?;

                    Ok(bookmark) as anyhow::Result<Bookmark>
                };

                // apply rules
                let with_rules =
                    Self::apply_rules(bookmark.id, self.bmark_mgr.clone(), self.config.clone())?
                        .ok_or_else(|| anyhow!("bookmark not found"))?;

                return with_meta.map(|_| with_rules);
            }
        } else {
            // if no metadata apply Rules.
            return Self::apply_rules(bookmark.id, self.bmark_mgr.clone(), self.config.clone())?
                .ok_or_else(|| anyhow!("bookmark not found"));
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
