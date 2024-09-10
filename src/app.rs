use crate::{
    bookmarks,
    config::Config,
    eid::Eid,
    metadata::{fetch_meta, MetaOptions, Metadata},
    parse_tags,
    rules::{self, Rule},
    scrape::guess_filetype,
    storage,
};
use anyhow::{anyhow, bail, Context};
use serde_json::json;
use std::{
    sync::{atomic::AtomicU16, mpsc, Arc, RwLock},
    thread::sleep,
    time::{Duration, SystemTime},
};

pub enum BmarkManagerBackend {
    Local(String),
    Remote(String),
}

pub trait AppBackend: Send + Sync {
    fn create(
        &self,
        bmark_create: bookmarks::BookmarkCreate,
        opts: AddOpts,
    ) -> anyhow::Result<bookmarks::Bookmark>;
    fn update(
        &self,
        id: u64,
        bmark_update: bookmarks::BookmarkUpdate,
    ) -> anyhow::Result<Option<bookmarks::Bookmark>>;
    fn delete(&self, id: u64) -> anyhow::Result<Option<bool>>;
    fn search_delete(&self, query: bookmarks::SearchQuery) -> anyhow::Result<usize>;
    fn search_update(
        &self,
        query: bookmarks::SearchQuery,
        bmark_update: bookmarks::BookmarkUpdate,
    ) -> anyhow::Result<usize>;
    fn total(&self) -> anyhow::Result<usize>;
    fn search(&self, query: bookmarks::SearchQuery) -> anyhow::Result<Vec<bookmarks::Bookmark>>;
}

pub struct AppDaemon {
    pub bmark_mgr: Arc<dyn bookmarks::BookmarkManager>,
    storage_mgr: Arc<dyn storage::StorageManager>,

    task_tx: Option<Arc<mpsc::Sender<Task>>>,
    task_queue_handle: Option<std::thread::JoinHandle<()>>,

    config: Arc<RwLock<Config>>,

    bmarks_last_modified: Arc<RwLock<SystemTime>>,
}

pub fn bmarks_modtime() -> SystemTime {
    use std::path::Path;
    let meta = std::fs::metadata(Path::new("bookmarks.json"));
    if let Err(err) = &meta {
        match err.kind() {
            std::io::ErrorKind::NotFound => return SystemTime::now(),
            _ => {}
        }
    };

    let bookmarks_metadata = meta.expect("couldnt read bookmarks.json");
    bookmarks_metadata
        .modified()
        .expect("couldnt get bookmarks.json modtime")
}

impl AppDaemon {
    pub fn run_queue(&mut self) {
        let (task_tx, task_rx) = mpsc::channel::<Task>();
        let handle = std::thread::spawn({
            let bmark_mgr = self.bmark_mgr.clone();
            let storage_mgr = self.storage_mgr.clone();
            let config = self.config.clone();

            move || {
                Self::start_queue(task_rx, bmark_mgr, storage_mgr, config);
            }
        });

        self.task_queue_handle = Some(handle);
        self.task_tx = Some(Arc::new(task_tx));
    }

    pub fn new(config: Arc<RwLock<Config>>, backend: BmarkManagerBackend) -> Self {
        let bmark_mgr: Arc<dyn bookmarks::BookmarkManager> = match backend {
            BmarkManagerBackend::Local(path) => {
                let mgr = Arc::new(bookmarks::BackendCsv::load(&path).unwrap());
                mgr.save();
                mgr
            }
            BmarkManagerBackend::Remote(addr) => {
                Arc::new(bookmarks::BackendHttp::load(&addr).unwrap())
            }
        };

        let storage_mgr = Arc::new(storage::BackendLocal::new("./uploads"));

        Self {
            bmark_mgr,
            storage_mgr,
            task_tx: None,
            task_queue_handle: None,
            config,
            bmarks_last_modified: Arc::new(RwLock::new(bmarks_modtime())),
        }
    }
}

pub enum Task {
    /// request to refetch metadata for a given bookmark
    FetchMetadata {
        bmark_id: u64,
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

impl AppBackend for AppDaemon {
    fn create(
        &self,
        bmark_create: bookmarks::BookmarkCreate,
        opts: AddOpts,
    ) -> anyhow::Result<bookmarks::Bookmark> {
        let url = bmark_create.url.clone();

        if !self.config.read().unwrap().allow_duplicates {
            let query = bookmarks::SearchQuery {
                url: Some(bmark_create.url.clone()),
                exact: true,
                limit: Some(1),
                ..Default::default()
            };

            if let Some(b) = self.search(query)?.first() {
                bail!("bookmark with this url already exists at index {0}", b.id);
            };
        }

        // create empty bookmark
        let bmark = self.bmark_mgr.create(bmark_create)?;

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

                    Ok(bmark) as anyhow::Result<bookmarks::Bookmark>
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

    fn update(
        &self,
        id: u64,
        bmark_update: bookmarks::BookmarkUpdate,
    ) -> anyhow::Result<Option<bookmarks::Bookmark>> {
        self.bmark_mgr.update(id, bmark_update)
    }

    fn delete(&self, id: u64) -> anyhow::Result<Option<bool>> {
        self.bmark_mgr.delete(id)
    }

    fn search_delete(&self, query: bookmarks::SearchQuery) -> anyhow::Result<usize> {
        self.bmark_mgr.search_delete(query)
    }

    fn search_update(
        &self,
        query: bookmarks::SearchQuery,
        bmark_update: bookmarks::BookmarkUpdate,
    ) -> anyhow::Result<usize> {
        self.bmark_mgr.search_update(query, bmark_update)
    }

    fn total(&self) -> anyhow::Result<usize> {
        self.bmark_mgr.total()
    }

    fn search(&self, query: bookmarks::SearchQuery) -> anyhow::Result<Vec<bookmarks::Bookmark>> {
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
}

impl AppDaemon {
    pub fn lazy_refresh_backend(&self) -> anyhow::Result<()> {
        let modtime = bmarks_modtime();
        let mut last_modified = self.bmarks_last_modified.write().unwrap();
        if *last_modified != modtime {
            *last_modified = modtime;
            self.bmark_mgr.refresh()?;
        }

        Ok(())
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

    fn merge_metadata(
        bookmark: bookmarks::Bookmark,
        meta: Metadata,
        storage_mgr: Arc<dyn storage::StorageManager>,
        bmark_mgr: Arc<dyn bookmarks::BookmarkManager>,
    ) -> anyhow::Result<Option<bookmarks::Bookmark>> {
        let mut bmark_update = bookmarks::BookmarkUpdate {
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
        bmark_mgr: Arc<dyn bookmarks::BookmarkManager>,
        rules: &Vec<Rule>,
    ) -> anyhow::Result<Option<bookmarks::Bookmark>> {
        let query = bookmarks::SearchQuery {
            id: Some(id),
            ..Default::default()
        };

        let bmark = bmark_mgr
            .search(query)
            .map(|b| b.first().cloned())?
            .ok_or_else(|| anyhow!("bookmark not found"))?;

        let mut bmark_update = bookmarks::BookmarkUpdate {
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

        for rule in rules.iter() {
            // recreating query because it could've been changed by previous rule
            let record = rules::Record {
                url: bmark_update.url.clone().unwrap_or(bmark.url.clone()),
                title: bmark_update.title.clone(),
                description: bmark_update.description.clone(),
                tags: bmark_update.tags.clone(),
                ..Default::default()
            };

            if !rule.is_match(&record) {
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

    pub fn start_queue(
        task_rx: mpsc::Receiver<Task>,
        bmark_mgr: Arc<dyn bookmarks::BookmarkManager>,
        storage_mgr: Arc<dyn storage::StorageManager>,
        config: Arc<RwLock<Config>>,
    ) {
        use std::sync::atomic::Ordering;

        let thread_ctr = Arc::new(AtomicU16::new(0));
        let max_threads = config.read().unwrap().task_queue_max_threads;

        println!("waiting for job");
        while let Ok(task) = task_rx.recv() {
            println!("got the job");
            let storage_mgr = storage_mgr.clone();
            let bmark_mgr = bmark_mgr.clone();
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
                        Task::FetchMetadata { bmark_id, opts } => {
                            thread_counter.fetch_add(1, Ordering::Relaxed);

                            let handle_metadata = || {
                                println!("picked up a job...");
                                let bookmarks = bmark_mgr.search(bookmarks::SearchQuery {
                                    id: Some(bmark_id),
                                    ..Default::default()
                                })?;
                                let bmark = bookmarks
                                    .first()
                                    .ok_or_else(|| anyhow!("bookmark {bmark_id} not found"))?;

                                let meta = Self::fetch_metadata(&bmark.url, opts)?;

                                let bmark = Self::merge_metadata(
                                    bmark.clone(),
                                    meta,
                                    storage_mgr.clone(),
                                    bmark_mgr.clone(),
                                )?
                                .context("bookmark {id} not found")?;

                                Ok(bmark) as anyhow::Result<bookmarks::Bookmark>
                            };

                            let _ = handle_metadata();
                            let rules = &config.read().unwrap().rules;
                            let _ = Self::apply_rules(bmark_id, bmark_mgr.clone(), &rules)
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
        if let Err(err) = self.task_tx.as_ref().unwrap().send(Task::Shutdown) {
            eprintln!("{err}");
        }
    }

    fn schedule_fetch_and_update_metadata(
        &self,
        bookmark: &bookmarks::Bookmark,
        meta_opts: FetchMetadataOpts,
    ) {
        if let Err(err) = self.task_tx.as_ref().unwrap().send(Task::FetchMetadata {
            bmark_id: bookmark.id,
            opts: meta_opts,
        }) {
            eprintln!("{err}");
        };
    }
}

impl AppDaemon {
    #[cfg(test)]
    pub fn new_with(
        bmark_mgr: Arc<dyn bookmarks::BookmarkManager>,
        storage_mgr: Arc<dyn storage::StorageManager>,
        task_tx: Arc<mpsc::Sender<Task>>,
        task_queue_handle: Option<std::thread::JoinHandle<()>>,
        config: Arc<RwLock<Config>>,
    ) -> Self {
        Self {
            bmark_mgr,
            storage_mgr,
            task_tx: Some(task_tx),
            task_queue_handle,
            config,
            bmarks_last_modified: Arc::new(RwLock::new(bmarks_modtime())),
        }
    }

    #[cfg(test)]
    pub fn config(&self) -> Arc<RwLock<Config>> {
        self.config.clone()
    }
}

pub struct AppRemote {
    remote_addr: String,
}

impl AppRemote {
    pub fn new(addr: &str) -> AppRemote {
        let remote_addr = addr.strip_suffix("/").unwrap_or(addr).to_string();

        AppRemote { remote_addr }
    }
}

impl AppBackend for AppRemote {
    fn create(
        &self,
        bmark_create: bookmarks::BookmarkCreate,
        opts: AddOpts,
    ) -> anyhow::Result<bookmarks::Bookmark> {
        let bmark: bookmarks::Bookmark = reqwest::blocking::Client::new()
            .put(format!("{}/api/bookmarks", self.remote_addr))
            .json(&json!({
                "title": bmark_create.title,
                "description": bmark_create.description,
                "tags": bmark_create.tags.map(|t| t.join(",")),
                "url": bmark_create.url,
                "async_meta": opts.async_meta,
                "no_meta": opts.meta_opts.is_some(),
                "no_headless": opts.meta_opts.unwrap_or_default().no_headless,
            }))
            .send()?
            .json()?;

        Ok(bmark)
    }

    fn update(
        &self,
        id: u64,
        bmark_update: bookmarks::BookmarkUpdate,
    ) -> anyhow::Result<Option<bookmarks::Bookmark>> {
        let bmark: bookmarks::Bookmark = reqwest::blocking::Client::new()
            .post(format!("{}/api/bookmarks/{}", self.remote_addr, id))
            .json(&json!({
                "title": bmark_update.title,
                "description": bmark_update.description,
                "tags": bmark_update.tags.map(|t| t.join(",")),
                "url": bmark_update.url,
            }))
            .send()?
            .json()?;

        Ok(Some(bmark))
    }

    fn delete(&self, id: u64) -> anyhow::Result<Option<bool>> {
        reqwest::blocking::Client::new()
            .delete(format!("{}/api/bookmarks/{}", self.remote_addr, id))
            .send()?
            .json()?;

        Ok(Some(true))
    }

    fn search_delete(&self, query: bookmarks::SearchQuery) -> anyhow::Result<usize> {
        let count: usize = reqwest::blocking::Client::new()
            .post(format!("{}//api/bookmarks/search_delete", self.remote_addr))
            .json(&query)
            .send()?
            .json()?;

        Ok(count)
    }

    fn search_update(
        &self,
        query: bookmarks::SearchQuery,
        bmark_update: bookmarks::BookmarkUpdate,
    ) -> anyhow::Result<usize> {
        let count: usize = reqwest::blocking::Client::new()
            .post(format!("{}//api/bookmarks/search_delete", self.remote_addr))
            .json(&json!({
                "query": query,
                "update": bmark_update,
            }))
            .send()?
            .json()?;

        Ok(count)
    }

    fn total(&self) -> anyhow::Result<usize> {
        let count: usize = reqwest::blocking::Client::new()
            .get(format!("{}//api/bookmarks/total", self.remote_addr))
            .send()?
            .json()?;

        Ok(count)
    }

    fn search(&self, query: bookmarks::SearchQuery) -> anyhow::Result<Vec<bookmarks::Bookmark>> {
        let bmarks: Vec<bookmarks::Bookmark> = reqwest::blocking::Client::new()
            .get(format!("{}//api/bookmarks", self.remote_addr))
            .json(&json!({
                "id": query.id,
                "title": query.title,
                "url": query.url,
                "description": query.description,
                "tags": query.tags.map(|tags| tags.join(",")),
                "exact": query.exact,
                "descending": false,
            }))
            .send()?
            .json()?;

        Ok(bmarks)
    }
}
