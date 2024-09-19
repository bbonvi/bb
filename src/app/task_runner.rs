use std::{
    sync::{
        atomic::{AtomicU16, Ordering},
        mpsc, Arc, RwLock,
    },
    thread::sleep,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::{anyhow, Context};
use serde::{Deserialize, Serialize};

use crate::{
    app::{backend::FetchMetadataOpts, local::AppLocal},
    bookmarks,
    config::Config,
    eid::Eid,
    storage::{self, StorageManager},
};

pub fn now() -> u128 {
    let start = SystemTime::now();
    let since_the_epoch = start
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards");
    since_the_epoch.as_millis()
}

pub fn throttle(counter: Arc<AtomicU16>, config: Arc<RwLock<Config>>) {
    while counter.load(Ordering::Relaxed) >= config.read().unwrap().task_queue_max_threads {
        sleep(Duration::from_millis(100));
    }
}

pub fn start_queue(
    task_rx: mpsc::Receiver<Task>,
    bmark_mgr: Arc<dyn bookmarks::BookmarkManager>,
    storage_mgr: Arc<dyn storage::StorageManager>,
    config: Arc<RwLock<Config>>,
) {
    use std::sync::atomic::Ordering;

    let thread_ctr = Arc::new(AtomicU16::new(0));

    log::debug!("waiting for job");
    while let Ok(task) = task_rx.recv() {
        log::debug!("got the job");
        let storage_mgr = storage_mgr.clone();
        let bmark_mgr = bmark_mgr.clone();
        let thread_counter = thread_ctr.clone();

        let config = config.clone();

        // graceful shutdown
        match &task {
            Task::Shutdown => {
                log::info!("{}", thread_counter.load(Ordering::Relaxed));
                while thread_counter.load(Ordering::Relaxed) > 0 {
                    sleep(Duration::from_millis(100));
                }
                return;
            }
            _ => {}
        };

        let id = save_task(task.clone(), Status::Pending);
        let task_handle = std::thread::spawn({
            let thread_counter = thread_counter.clone();
            let id = id.clone();
            move || {
                throttle(thread_counter.clone(), config.clone());

                thread_counter.fetch_add(1, Ordering::Relaxed);
                set_status(id.clone(), Status::InProgress);

                let status = task.run(bmark_mgr.clone(), storage_mgr.clone(), config.clone());

                set_status(id.clone(), status.clone());

                // remove task a bit later to give client an opportunity to react
                std::thread::spawn(move || {
                    sleep(Duration::from_secs(10));
                    remove_task(id);
                });
            }
        });

        // handle thread panics
        std::thread::spawn(move || {
            if let Err(err) = task_handle.join() {
                log::error!("task_handle panicked: {err:?}");
                remove_task(id);
            }

            thread_counter.fetch_sub(1, Ordering::Relaxed);
        });
    }
}

pub fn read_queue_dump() -> QueueDump {
    let store = storage::BackendLocal::new("./");

    let filename = "task-queue.json";

    if store.exists(filename) {
        serde_json::from_slice(&store.read(filename)).unwrap()
    } else {
        QueueDump {
            queue: vec![],
            now: now(),
        }
    }
}

pub fn write_queue_dump(queue_dump: &QueueDump) {
    let store = storage::BackendLocal::new("./");

    let filename = "task-queue.json";

    let queue_dump_str = serde_json::to_string_pretty(&queue_dump).unwrap();
    store.write(&filename, &queue_dump_str.as_bytes());
}

pub fn remove_task(id: Eid) {
    let mut queue_dump = read_queue_dump();
    queue_dump.queue.retain(|td| td.id != id);
    queue_dump.now = now();
    write_queue_dump(&queue_dump);
}

pub fn set_status(id: Eid, status: Status) {
    let mut queue_dump = read_queue_dump();
    if let Some(task_dump) = queue_dump.queue.iter_mut().find(|td| td.id == id) {
        task_dump.status = status;
    }

    queue_dump.now = now();
    write_queue_dump(&queue_dump);
}

pub fn save_task(task: Task, status: Status) -> Eid {
    let eid = Eid::new();

    let task_dump = TaskDump {
        id: eid.clone(),
        task,
        status,
    };

    let mut queue_dump = read_queue_dump();

    queue_dump.queue.push(task_dump);
    queue_dump.now = now();
    write_queue_dump(&queue_dump);

    eid
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Status {
    Interrupted,
    Pending,
    InProgress,
    Done,
    Error(String),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct QueueDump {
    pub queue: Vec<TaskDump>,
    pub now: u128,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TaskDump {
    pub id: Eid,
    pub task: Task,
    pub status: Status,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Task {
    /// request to refetch metadata for a given bookmark
    FetchMetadata {
        bmark_id: u64,
        opts: FetchMetadataOpts,
    },

    /// request to gracefully shutdown task queue
    Shutdown,
}

impl Task {
    pub fn run(
        &self,
        bmark_mgr: Arc<dyn bookmarks::BookmarkManager>,
        storage_mgr: Arc<dyn storage::StorageManager>,
        config: Arc<RwLock<Config>>,
    ) -> Status {
        match self {
            Task::FetchMetadata { bmark_id, opts } => {
                let bmark_id = *bmark_id;
                let handle_metadata = || {
                    log::debug!("picked up a job...");
                    let bookmarks = bmark_mgr.search(bookmarks::SearchQuery {
                        id: Some(bmark_id),
                        ..Default::default()
                    })?;
                    let bmark = bookmarks
                        .first()
                        .ok_or_else(|| anyhow!("bookmark {bmark_id} not found"))?;

                    let meta = AppLocal::fetch_metadata(&bmark.url, opts.clone())?;

                    let bmark = AppLocal::merge_metadata(
                        bmark.clone(),
                        meta,
                        storage_mgr.clone(),
                        bmark_mgr.clone(),
                    )?
                    .context("bookmark {id} not found")?;

                    Ok(bmark) as anyhow::Result<bookmarks::Bookmark>
                };

                let fetch_meta_result = handle_metadata();

                let rules = &config.read().unwrap().rules;
                match AppLocal::apply_rules(bmark_id, bmark_mgr.clone(), &rules) {
                    Ok(_) => match fetch_meta_result {
                        Ok(_) => Status::Done,
                        Err(err) => Status::Error(err.to_string()),
                    },
                    Err(err) => Status::Error(err.to_string()),
                }
            }
            Task::Shutdown => unreachable!(),
        }
    }
}
