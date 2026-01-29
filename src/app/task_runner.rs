use std::{
    sync::{
        atomic::{AtomicU16, Ordering},
        mpsc, Arc, RwLock,
    },
    thread::sleep,
    time::{Duration, SystemTime, UNIX_EPOCH},
};

use anyhow::anyhow;
use rand::random;
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
        if let Task::Shutdown = &task {
            log::info!("{}", thread_counter.load(Ordering::Relaxed));
            while thread_counter.load(Ordering::Relaxed) > 0 {
                sleep(Duration::from_millis(100));
            }
            return;
        };

        let id = save_task(task.clone(), Status::Pending);
        let task_handle = std::thread::spawn({
            let thread_counter = thread_counter.clone();
            let id = id.clone();
            move || {
                throttle(thread_counter.clone(), config.clone());

                thread_counter.fetch_add(1, Ordering::Relaxed);
                set_status(id.clone(), Status::InProgress);

                let max_retries = config.read().unwrap().task_queue_max_retries;
                let mut attempt = 0u8;

                loop {
                    let status = task.run(bmark_mgr.clone(), storage_mgr.clone(), config.clone());

                    match &status {
                        Status::Error(msg) if attempt < max_retries && is_retryable_error(msg) => {
                            attempt += 1;
                            let delay_ms = 5000 * 2u64.pow(attempt as u32 - 1) + rand_jitter();
                            log::info!(
                                "task {}: retrying (attempt {}/{}) after error: {}, backoff {}ms",
                                id,
                                attempt,
                                max_retries,
                                msg,
                                delay_ms
                            );
                            set_attempt(id.clone(), attempt);
                            set_status(id.clone(), Status::Pending);
                            sleep(Duration::from_millis(delay_ms));
                        }
                        _ => {
                            set_status(id.clone(), status);
                            break;
                        }
                    }
                }

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
    let store = match storage::BackendLocal::new("./") {
        Ok(s) => s,
        Err(e) => {
            log::error!("failed to initialize queue storage: {e}");
            return QueueDump {
                queue: vec![],
                now: now(),
            };
        }
    };

    let filename = "task-queue.json";

    if store.exists(filename) {
        match store.read(filename) {
            Ok(data) => serde_json::from_slice(&data).unwrap(),
            Err(e) => {
                log::error!("failed to read queue dump: {e}");
                QueueDump {
                    queue: vec![],
                    now: now(),
                }
            }
        }
    } else {
        QueueDump {
            queue: vec![],
            now: now(),
        }
    }
}

pub fn write_queue_dump(queue_dump: &QueueDump) {
    let store = match storage::BackendLocal::new("./") {
        Ok(s) => s,
        Err(e) => {
            log::error!("failed to initialize queue storage: {e}");
            return;
        }
    };

    let filename = "task-queue.json";

    let queue_dump_str = serde_json::to_string_pretty(&queue_dump).unwrap();
    if let Err(e) = store.write(filename, queue_dump_str.as_bytes()) {
        log::error!("failed to write queue dump: {e}");
    }
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

fn set_attempt(id: Eid, attempt: u8) {
    let mut queue_dump = read_queue_dump();
    if let Some(task_dump) = queue_dump.queue.iter_mut().find(|td| td.id == id) {
        task_dump.attempt = attempt;
    }
    queue_dump.now = now();
    write_queue_dump(&queue_dump);
}


fn is_retryable_error(msg: &str) -> bool {
    let msg_lower = msg.to_lowercase();

    // Check for retryable patterns
    let retryable = msg_lower.contains("timeout")
        || msg_lower.contains("timed out")
        || msg_lower.contains("connection")
        || msg_lower.contains("reset by peer")
        || msg_lower.contains("500")
        || msg_lower.contains("502")
        || msg_lower.contains("503")
        || msg_lower.contains("504");

    // Exclude 4xx errors (client errors are generally not retryable)
    let is_client_error = msg_lower.contains("400")
        || msg_lower.contains("401")
        || msg_lower.contains("403")
        || msg_lower.contains("404")
        || msg_lower.contains("405")
        || msg_lower.contains("406")
        || msg_lower.contains("407")
        || msg_lower.contains("408")
        || msg_lower.contains("409")
        || msg_lower.contains("410")
        || msg_lower.contains("411")
        || msg_lower.contains("412")
        || msg_lower.contains("413")
        || msg_lower.contains("414")
        || msg_lower.contains("415")
        || msg_lower.contains("416")
        || msg_lower.contains("417")
        || msg_lower.contains("418")
        || msg_lower.contains("421")
        || msg_lower.contains("422")
        || msg_lower.contains("423")
        || msg_lower.contains("424")
        || msg_lower.contains("425")
        || msg_lower.contains("426")
        || msg_lower.contains("428")
        || msg_lower.contains("429")
        || msg_lower.contains("431")
        || msg_lower.contains("451");

    retryable && !is_client_error
}

fn rand_jitter() -> u64 {
    random::<u64>() % 2000
}

pub fn save_task(task: Task, status: Status) -> Eid {
    let eid = Eid::new();

    let task_dump = TaskDump {
        id: eid.clone(),
        task,
        status,
        attempt: 0,
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
    #[serde(default)]
    pub attempt: u8,
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

                    // Inject scrape_config from current config into opts
                    let mut opts = opts.clone();
                    let scrape_cfg = config.read().unwrap().scrape.clone();
                    opts.meta_opts.scrape_config = Some(scrape_cfg);

                    let meta = AppLocal::fetch_metadata(&bmark.url, opts)?;

                    let img_config = &config.read().unwrap().images;
                    let bmark = AppLocal::merge_metadata(
                        bmark.clone(),
                        meta,
                        storage_mgr.clone(),
                        bmark_mgr.clone(),
                        img_config,
                    )?;

                    Ok(bmark) as anyhow::Result<bookmarks::Bookmark>
                };

                let fetch_meta_result = handle_metadata();

                let rules = &config.read().unwrap().rules;
                match AppLocal::apply_rules(bmark_id, bmark_mgr.clone(), rules) {
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
