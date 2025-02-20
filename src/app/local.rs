use crate::{
    bookmarks,
    config::Config,
    eid::Eid,
    metadata::{fetch_meta, Metadata},
    rules::{self, Rule},
    storage::{self, BackendLocal},
};

use super::task_runner::{self, Status, Task};
use anyhow::{anyhow, Context};
use homedir::my_home;
use std::{
    collections::HashMap,
    sync::{mpsc, Arc, RwLock},
    time::SystemTime,
};

use super::{backend::*, errors::AppError};

pub struct AppLocal {
    pub bmark_mgr: Arc<dyn bookmarks::BookmarkManager>,
    tags_cache: Arc<RwLock<Vec<String>>>,
    pub storage_mgr: Arc<dyn storage::StorageManager>,

    task_tx: Option<Arc<mpsc::Sender<Task>>>,
    task_queue_handle: Option<std::thread::JoinHandle<()>>,

    config: Arc<RwLock<Config>>,

    bmarks_last_modified: Arc<RwLock<SystemTime>>,
}

pub fn bmarks_modtime() -> SystemTime {
    use std::path::Path;

    let base_path = std::env::var("BB_BASE_PATH").unwrap_or(format!(
        "{}/.local/share/bb",
        my_home()
            .expect("couldnt find home dir")
            .expect("couldnt find home dir")
            .to_string_lossy()
    ));

    let bookmarks_path = format!("{base_path}/bookmarks.csv");

    let meta = std::fs::metadata(Path::new(&bookmarks_path));
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

impl AppLocal {
    pub fn run_queue(&mut self) {
        let (task_tx, task_rx) = mpsc::channel::<Task>();

        let handle = std::thread::spawn({
            let bmark_mgr = self.bmark_mgr.clone();
            let storage_mgr = self.storage_mgr.clone();
            let config = self.config.clone();

            let mut queue_dump = task_runner::read_queue_dump();
            let task_list = queue_dump.queue.clone();

            queue_dump.queue = Vec::new();
            task_runner::write_queue_dump(&queue_dump);

            std::thread::spawn({
                let task_tx = task_tx.clone();

                move || {
                    for task in task_list {
                        if let Status::Done = task.status {
                            continue;
                        }

                        log::info!("restarting interrupted task \"{:?}\"", task.task);
                        if let Err(err) = task_tx.send(task.task) {
                            log::error!("failed to initialize interrupted task: {err:?}");
                        }
                    }
                }
            });

            move || {
                task_runner::start_queue(task_rx, bmark_mgr, storage_mgr, config);
            }
        });

        self.task_queue_handle = Some(handle);
        self.task_tx = Some(Arc::new(task_tx));
    }

    pub fn new(config: Arc<RwLock<Config>>, path: &str, storage_mgr: BackendLocal) -> Self {
        let bmark_mgr = Arc::new(bookmarks::BackendCsv::load(&path).unwrap());
        let storage_mgr = Arc::new(storage_mgr);

        bmark_mgr.save();

        Self {
            bmark_mgr,
            storage_mgr,
            task_tx: None,
            tags_cache: Arc::new(RwLock::new(Vec::new())),
            task_queue_handle: None,
            config,
            bmarks_last_modified: Arc::new(RwLock::new(bmarks_modtime())),
        }
    }
}

impl AppBackend for AppLocal {
    fn refresh_metadata(&self, id: u64, opts: RefreshMetadataOpts) -> anyhow::Result<(), AppError> {
        let bmarks = self.bmark_mgr.search(bookmarks::SearchQuery {
            id: Some(id),
            ..Default::default()
        })?;

        let bmark = bmarks.first().ok_or(anyhow!("not found"))?;

        if opts.async_meta {
            self.schedule_fetch_and_update_metadata(
                &bmark,
                FetchMetadataOpts {
                    no_https_upgrade: true,
                    meta_opts: opts.meta_opts.clone(),
                },
            );
        } else {
            // attempt to fetch and merge metadata
            {
                let meta = Self::fetch_metadata(
                    &bmark.url,
                    FetchMetadataOpts {
                        no_https_upgrade: true,
                        meta_opts: opts.meta_opts.clone(),
                    },
                )?;

                Self::merge_metadata(
                    bmark.clone(),
                    meta,
                    self.storage_mgr.clone(),
                    self.bmark_mgr.clone(),
                )?
                .context("bmark not found")?;
            };

            // apply rules
            let rules = &self.config.read().unwrap().rules;
            Self::apply_rules(bmark.id, self.bmark_mgr.clone(), &rules)?
                .ok_or_else(|| anyhow!("bmark not found"))?;
        };

        Self::schedule_tags_cache_reval(self.bmark_mgr.clone(), self.tags_cache.clone());

        Ok(())
    }

    fn create(
        &self,
        bmark_create: bookmarks::BookmarkCreate,
        opts: AddOpts,
    ) -> anyhow::Result<bookmarks::Bookmark, AppError> {
        let url = bmark_create.url.clone();

        let query = bookmarks::SearchQuery {
            url: Some(bmark_create.url.clone()),
            exact: true,
            limit: Some(1),
            ..Default::default()
        };

        // querying manager directly because we need to avoid
        // hidden_by_default functionality
        // if let Some(b) = self.bmark_mgr.search(query)?.first() {
        //     return Err(AppError::AlreadyExists(b.id));
        // }
        if let Some(b) = self.search(query)?.first() {
            return Err(AppError::AlreadyExists(b.id));
        };

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
                if !opts.skip_rules {
                    let rules = &self.config.read().unwrap().rules;
                    let with_rules = Self::apply_rules(bmark.id, self.bmark_mgr.clone(), &rules)?
                        .ok_or_else(|| anyhow!("bmark not found"))?;
                    return Ok(with_meta.map(|_| with_rules)?);
                }

                return Ok(with_meta?);
            }
        } else if !opts.skip_rules {
            // if no metadata apply Rules.
            let rules = &self.config.read().unwrap().rules;
            return Ok(Self::apply_rules(bmark.id, self.bmark_mgr.clone(), &rules)?
                .ok_or(AppError::NotFound)?);
        }

        Self::schedule_tags_cache_reval(self.bmark_mgr.clone(), self.tags_cache.clone());

        Ok(bmark)
    }

    fn update(
        &self,
        id: u64,
        bmark_update: bookmarks::BookmarkUpdate,
    ) -> anyhow::Result<bookmarks::Bookmark, AppError> {
        if bmark_update.url.is_some() {
            if let Some(b) = self
                .search(bookmarks::SearchQuery {
                    url: bmark_update.url.clone(),
                    exact: true,
                    ..Default::default()
                })?
                .iter()
                .filter(|b| b.id != id)
                .collect::<Vec<_>>()
                .first()
            {
                log::info!("already exists{id}");
                return Err(AppError::AlreadyExists(b.id));
            };
        }

        let bmark = self
            .bmark_mgr
            .update(id, bmark_update)?
            .ok_or(AppError::NotFound)?;

        Self::schedule_tags_cache_reval(self.bmark_mgr.clone(), self.tags_cache.clone());

        Ok(bmark)
    }

    fn delete(&self, id: u64) -> anyhow::Result<(), AppError> {
        self.bmark_mgr
            .delete(id)?
            .ok_or(AppError::NotFound)
            .map(|_| ())?;

        Self::schedule_tags_cache_reval(self.bmark_mgr.clone(), self.tags_cache.clone());

        Ok(())
    }

    fn search_delete(&self, query: bookmarks::SearchQuery) -> anyhow::Result<usize, AppError> {
        let search_delete = self.bmark_mgr.search_delete(query)?;
        Self::schedule_tags_cache_reval(self.bmark_mgr.clone(), self.tags_cache.clone());
        Ok(search_delete)
    }

    fn search_update(
        &self,
        query: bookmarks::SearchQuery,
        bmark_update: bookmarks::BookmarkUpdate,
    ) -> anyhow::Result<usize, AppError> {
        let search_update = self.bmark_mgr.search_update(query, bmark_update)?;

        Self::schedule_tags_cache_reval(self.bmark_mgr.clone(), self.tags_cache.clone());

        Ok(search_update)
    }

    fn total(&self) -> anyhow::Result<usize, AppError> {
        Ok(self.bmark_mgr.total()?)
    }

    fn search(
        &self,
        query: bookmarks::SearchQuery,
    ) -> anyhow::Result<Vec<bookmarks::Bookmark>, AppError> {
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

            // TODO: this was here initially but it is introduces weird behaviour.
            // // hidden by default functionality
            // let config = self.config.read().unwrap();
            // let hidden_by_default = &config.hidden_by_default;
            // if !hidden_by_default.is_empty() {
            //     let mut query_tags = query.tags.clone().unwrap_or_default();
            //
            //     let mut append = Vec::new();
            //     for hidden_tag in hidden_by_default {
            //         if query_tags
            //             .iter()
            //             .find(|query_tag| {
            //                 **query_tag == *hidden_tag
            //                     || query_tag.starts_with(&format!("{hidden_tag}/"))
            //             })
            //             .is_none()
            //         {
            //             append.push(format!("-{hidden_tag}"))
            //         }
            //     }
            //
            //     query_tags.append(&mut append);
            //
            //     query.tags = Some(query_tags);
            // }
        }

        Ok(self.bmark_mgr.search(query)?)
    }

    fn tags(&self) -> anyhow::Result<Vec<String>, AppError> {
        if self.tags_cache.read().unwrap().is_empty() {
            Self::tags_cache_reeval(self.bmark_mgr.clone(), self.tags_cache.clone())?;
        }

        Ok(self.tags_cache.read().unwrap().to_vec())
    }

    fn upload_cover(
        &self,
        id: u64,
        file: Vec<u8>,
    ) -> anyhow::Result<bookmarks::Bookmark, AppError> {
        let bmarks = self.bmark_mgr.search(bookmarks::SearchQuery {
            id: Some(id),
            ..Default::default()
        })?;
        let bmark = bmarks.first().ok_or(AppError::NotFound)?;

        let mut bmark_update = bookmarks::BookmarkUpdate {
            ..Default::default()
        };

        let filetype = infer::get(&file)
            .map(|ftype| ftype.extension())
            .unwrap_or("png")
            .to_string();

        let image_id = format!("{}.{}", Eid::new(), filetype);

        self.storage_mgr.write(&image_id, &file);

        bmark_update.image_id = Some(image_id.to_string());

        if let Some(old_image) = &bmark.image_id {
            if self.storage_mgr.exists(&old_image) {
                self.storage_mgr.delete(&old_image);
            }
        }

        self.bmark_mgr
            .update(id, bmark_update)?
            .ok_or(AppError::NotFound)
    }

    fn upload_icon(&self, id: u64, file: Vec<u8>) -> anyhow::Result<bookmarks::Bookmark, AppError> {
        let bmarks = self.bmark_mgr.search(bookmarks::SearchQuery {
            id: Some(id),
            ..Default::default()
        })?;
        let bmark = bmarks.first().ok_or(AppError::NotFound)?;

        let mut bmark_update = bookmarks::BookmarkUpdate {
            ..Default::default()
        };

        let filetype = infer::get(&file)
            .map(|ftype| ftype.extension())
            .unwrap_or("png")
            .to_string();

        let icon_id = format!("{}.{}", Eid::new(), filetype);

        self.storage_mgr.write(&icon_id, &file);

        bmark_update.icon_id = Some(icon_id.to_string());

        if let Some(old_icon) = &bmark.icon_id {
            if self.storage_mgr.exists(&old_icon) {
                self.storage_mgr.delete(&old_icon);
            }
        }

        self.bmark_mgr
            .update(id, bmark_update)?
            .ok_or(AppError::NotFound)
    }
}

impl AppLocal {
    fn schedule_tags_cache_reval(
        bmark_mgr: Arc<dyn bookmarks::BookmarkManager>,
        tags_cache: Arc<RwLock<Vec<String>>>,
    ) {
        std::thread::spawn(move || {
            if let Err(err) = Self::tags_cache_reeval(bmark_mgr, tags_cache) {
                log::error!("{err}");
            }
        });
    }

    fn tags_cache_reeval(
        bmark_mgr: Arc<dyn bookmarks::BookmarkManager>,
        tags_cache: Arc<RwLock<Vec<String>>>,
    ) -> anyhow::Result<(), AppError> {
        log::info!("refreshing da cache");
        let bmarks = bmark_mgr.search(bookmarks::SearchQuery {
            ..Default::default()
        })?;

        let tags: Vec<String> = bmarks
            .into_iter()
            .map(|bmark| bmark.tags)
            .flatten()
            .collect();

        let mut counts = HashMap::new();
        for tag in &tags {
            *counts.entry(tag.clone()).or_insert(0) += 1;
        }

        let mut unique_tags: Vec<String> = counts.keys().cloned().collect();
        unique_tags.sort_by(|a, b| counts[b].cmp(&counts[a]));

        *tags_cache.write().unwrap() = unique_tags;

        Ok(())
    }

    pub fn lazy_refresh_backend(&self) -> anyhow::Result<()> {
        let mut last_modified = self.bmarks_last_modified.write().unwrap();
        let modtime = bmarks_modtime();

        if *last_modified < modtime {
            self.bmark_mgr.refresh()?;
            *last_modified = bmarks_modtime();
        }

        self.config().write().unwrap().reload();

        Ok(())
    }

    pub fn fetch_metadata(url: &str, opts: FetchMetadataOpts) -> anyhow::Result<Metadata> {
        let mut url_parsed = reqwest::Url::parse(&url).unwrap();
        let mut tried_https = false;
        if url_parsed.scheme() == "http" && !opts.no_https_upgrade {
            log::warn!("http url provided. trying https first");
            url_parsed.set_scheme("https").unwrap();
            tried_https = true;
        }

        let err = match fetch_meta(&url_parsed.to_string(), opts.meta_opts.clone()) {
            Ok(m) => return Ok(m),
            Err(err) => Err(err),
        };

        if tried_https {
            log::warn!("https attempt failed. trying http.");
            url_parsed.set_scheme("http").unwrap();
            return fetch_meta(&url_parsed.to_string(), opts.meta_opts.clone());
        }

        return err;
    }

    pub fn merge_metadata(
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

        if bookmark.image_id.is_none() {
            if let Some(ref image) = meta.image {
                let filetype = infer::get(&image)
                    .map(|ftype| ftype.extension())
                    .unwrap_or("png")
                    .to_string();

                let image_id = format!("{}.{}", Eid::new(), filetype);

                storage_mgr.write(&image_id, &image);
                bmark_update.image_id = Some(image_id.to_string());
            };
        }

        if bookmark.icon_id.is_none() {
            if let Some(ref icon) = meta.icon {
                let filetype = infer::get(&icon)
                    .map(|ftype| ftype.extension())
                    .unwrap_or("png")
                    .to_string();

                let icon_id = format!("{}.{}", Eid::new(), filetype);

                storage_mgr.write(&icon_id, &icon);
                bmark_update.icon_id = Some(icon_id.to_string());
            };
        }

        bmark_mgr.update(bookmark.id, bmark_update)
    }

    pub fn apply_rules(
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

    pub fn wait_task_queue_finish(&mut self) {
        self.task_queue_handle.take().unwrap().join().unwrap();
    }

    pub fn shutdown(&self) {
        if let Err(err) = self.task_tx.as_ref().unwrap().send(Task::Shutdown) {
            log::error!("{err}");
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
            log::error!("{err}");
        };
    }
}

impl AppLocal {
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
            tags_cache: Arc::new(RwLock::new(Vec::new())),
            task_queue_handle,
            config,
            bmarks_last_modified: Arc::new(RwLock::new(bmarks_modtime())),
        }
    }

    pub fn config(&self) -> Arc<RwLock<Config>> {
        self.config.clone()
    }
}
