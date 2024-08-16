use crate::{
    bookmarks::{
        Bookmark, BookmarkCreate, BookmarkMgrBackend, BookmarkMgrJson, BookmarkUpdate, SearchQuery,
    },
    config::Config,
    eid::Eid,
    metadata::{fetch_meta, MetaOptions, Metadata},
    rules::Rule,
    scrape::guess_filetype,
    storage::{StorageMgrBackend, StorageMgrLocal},
};
use anyhow::{anyhow, Context};
use std::{
    sync::{atomic::AtomicU16, mpsc, Arc, RwLock},
    thread::sleep,
    time::Duration,
};

pub struct App {
    pub bmark_mgr: Arc<dyn BookmarkMgrBackend>,
    pub storage_mgr: Arc<dyn StorageMgrBackend>,

    task_tx: Arc<mpsc::Sender<Task>>,
    task_queue_handle: Option<std::thread::JoinHandle<()>>,
    pub config: Arc<RwLock<Config>>,
}

impl App {
    pub fn new(config: Arc<RwLock<Config>>) -> Self {
        let bmark_mgr = Arc::new(BookmarkMgrJson::load());
        let storage_mgr = Arc::new(StorageMgrLocal::new("./uploads"));

        let (tx, rx) = mpsc::channel::<Task>();

        let handle = std::thread::spawn({
            let bmark_mgr = bmark_mgr.clone();
            let storage_mgr = storage_mgr.clone();

            let config = config.clone();
            move || {
                Self::start_queue(rx, bmark_mgr, storage_mgr, config);
            }
        });

        Self {
            bmark_mgr,
            storage_mgr,
            task_tx: Arc::new(tx),
            task_queue_handle: Some(handle),
            config,
        }
    }
}

pub enum Task {
    /// request to refetch metadata for a given bookmark
    FetchMetadata {
        bookmark_id: u64,
        opts: FetchMetadataOpts,
    },

    /// request to gracefully shutdown task queue
    Shutdown,
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
    pub fn search(&self, query: SearchQuery) -> anyhow::Result<Vec<Bookmark>> {
        let mut query = query;

        // TODO: do we prevent queries against empty strings?
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

        // create empty bookmark
        let bmark = self.bmark_mgr.add(bmark_create)?;

        // add metadata
        if let Some(meta_opts) = opts.meta_opts {
            if opts.async_meta {
                self.schedule_fetch_and_update_metadata(
                    &bmark,
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

                    let bmark = Self::merge_metadata(
                        bmark.clone(),
                        meta,
                        self.storage_mgr.clone(),
                        self.bmark_mgr.clone(),
                    )?
                    .context("bmark not found")?;

                    Ok(bmark) as anyhow::Result<Bookmark>
                };

                // apply rules
                let rules = &self.config.read().unwrap().rules;
                let with_rules = Self::apply_rules(bmark.id, self.bmark_mgr.clone(), &rules)?
                    .ok_or_else(|| anyhow!("bmark not found"))?;

                return with_meta.map(|_| with_rules);
            }
        } else {
            // if no metadata apply Rules.
            let rules = &self.config.read().unwrap().rules;
            return Self::apply_rules(bmark.id, self.bmark_mgr.clone(), &rules)?
                .ok_or_else(|| anyhow!("bmark not found"));
        }

        Ok(bmark)
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
        if let Err(err) = self.task_tx.send(Task::FetchMetadata {
            bookmark_id: bookmark.id,
            opts: meta_opts,
        }) {
            eprintln!("{err}");
        };
    }
}

impl App {
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

    fn apply_rules(
        id: u64,
        bmark_mgr: Arc<dyn BookmarkMgrBackend>,
        rules: &Vec<Rule>,
    ) -> anyhow::Result<Option<Bookmark>> {
        let query = SearchQuery {
            id: Some(id),
            ..Default::default()
        };

        let bmark = bmark_mgr
            .search(query)
            .map(|b| b.first().cloned())?
            .ok_or_else(|| anyhow!("bookmark not found"))?;

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
        for rule in rules.iter() {
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
}

impl App {
    fn start_queue(
        task_rx: mpsc::Receiver<Task>,
        bookmark_mgr: Arc<dyn BookmarkMgrBackend>,
        storage_mgr: Arc<dyn StorageMgrBackend>,
        config: Arc<RwLock<Config>>,
    ) {
        use std::sync::atomic::Ordering;

        let thread_ctr = Arc::new(AtomicU16::new(0));
        let max_threads = config.read().unwrap().task_queue_max_threads;

        while let Ok(task) = task_rx.recv() {
            let storage_mgr = storage_mgr.clone();
            let bookmark_mgr = bookmark_mgr.clone();
            let thread_counter = thread_ctr.clone();

            let config = config.clone();

            // graceful shutdown
            match &task {
                Task::Shutdown => {
                    while thread_counter.load(Ordering::Relaxed) > 0 {
                        sleep(Duration::from_millis(100));
                    }
                    return;
                }
                _ => {}
            };

            while thread_counter.load(Ordering::Relaxed) >= max_threads {
                sleep(Duration::from_millis(100));
            }

            let task_handle = std::thread::spawn({
                let thread_counter = thread_counter.clone();
                move || {
                    match task {
                        Task::FetchMetadata { bookmark_id, opts } => {
                            thread_counter.fetch_add(1, Ordering::Relaxed);

                            let handle_metadata = || {
                                println!("picked up a job...");
                                let bookmarks = bookmark_mgr.search(SearchQuery {
                                    id: Some(bookmark_id),
                                    ..Default::default()
                                })?;
                                let bmark = bookmarks
                                    .first()
                                    .ok_or_else(|| anyhow!("bookmark {bookmark_id} not found"))?;

                                let meta = Self::fetch_metadata(&bmark.url, opts)?;

                                let bmark = Self::merge_metadata(
                                    bmark.clone(),
                                    meta,
                                    storage_mgr.clone(),
                                    bookmark_mgr.clone(),
                                )?
                                .context("bookmark {id} not found")?;

                                Ok(bmark) as anyhow::Result<Bookmark>
                            };

                            let _ = handle_metadata();
                            let rules = &config.read().unwrap().rules;
                            let _ = Self::apply_rules(bookmark_id, bookmark_mgr.clone(), &rules)
                                .map_err(|err| eprintln!("{err}"));

                            thread_counter.fetch_sub(1, Ordering::Relaxed);
                        }
                        Task::Shutdown => unreachable!(),
                    };
                }
            });

            // handle thread panics
            // TODO: get rid of unwraps and delete this code.
            std::thread::spawn(move || {
                if let Err(err) = task_handle.join() {
                    eprintln!("{err:?}");
                    thread_counter.fetch_sub(1, Ordering::Relaxed);
                }
            });
        }
    }

    pub fn wait_task_queue_finish(&mut self) {
        self.task_queue_handle.take().unwrap().join().unwrap();
    }

    pub fn shutdown(&self) {
        if let Err(err) = self.task_tx.send(Task::Shutdown) {
            eprintln!("{err}");
        }
    }
}
