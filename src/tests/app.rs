use std::sync::mpsc;
use std::sync::Arc;
use std::sync::RwLock;

use crate::app::{AppBackend, AppLocal};
use crate::bookmarks;
use crate::storage;
use crate::task_runner::Task;

pub fn create_app() -> AppLocal {
    let _ = std::fs::remove_file("bookmarks-test.csv");
    let _ = std::fs::remove_file("config-test.yaml");

    let bmark_mgr = Arc::new(bookmarks::BackendCsv::load("bookmarks-test.csv").unwrap());
    let storage_mgr = Arc::new(storage::BackendLocal::new("./uploads"));

    let (task_tx, task_rx) = mpsc::channel::<Task>();

    let config = Arc::new(RwLock::new(crate::config::Config::load_with("config-test")));

    let handle = std::thread::spawn({
        let bmark_mgr = bmark_mgr.clone();
        let storage_mgr = storage_mgr.clone();
        let config = config.clone();

        move || {
            // AppLocal::start_queue(task_rx, bmark_mgr, storage_mgr, config);
        }
    });

    AppLocal::new_with(
        bmark_mgr,
        storage_mgr,
        Arc::new(task_tx),
        Some(handle),
        config,
    )
}

#[test]
pub fn test_create_bookmark() {
    let app = create_app();
    let bmark_create = bookmarks::BookmarkCreate {
        url: "https://example.com/lmao-what".to_string(),
        title: Some("lmao what".to_string()),
        ..Default::default()
    };

    let opts = crate::app::AddOpts {
        no_https_upgrade: false,
        async_meta: false,
        meta_opts: None,
        skip_rules: false,
    };

    let bookmark = app.create(bmark_create, opts).unwrap();

    assert_eq!(&bookmark.url, "https://example.com/lmao-what");
}

#[test]
pub fn test_bookmark_create() {
    let app = create_app();

    let opts = crate::app::AddOpts {
        no_https_upgrade: false,
        async_meta: false,
        meta_opts: None,
        skip_rules: false,
    };

    let bmark_create = bookmarks::BookmarkCreate {
        url: "https://example.com/2".to_string(),
        title: Some("lmao what2".to_string()),
        ..Default::default()
    };

    let bookmark = app.create(bmark_create.clone(), opts.clone()).unwrap();

    assert_eq!(&bookmark.url, "https://example.com/2");
    assert_eq!(&bookmark.title, "lmao what2");
}

#[test]
pub fn test_bookmark_search() {
    let app = create_app();

    let opts = crate::app::AddOpts {
        no_https_upgrade: false,
        async_meta: false,
        meta_opts: None,
        skip_rules: false,
    };

    for b in 0..10 {
        let bmark_create = bookmarks::BookmarkCreate {
            url: format!("https://example.com/{b}"),
            title: Some(format!("very cool title #{b}")),
            tags: Some(vec![format!("all"), format!("tag{b}")]),
            ..Default::default()
        };

        app.create(bmark_create.clone(), opts.clone()).unwrap();
    }

    {
        let query = bookmarks::SearchQuery {
            id: Some(0),
            ..Default::default()
        };

        assert_eq!(app.search(query).unwrap().len(), 1);
    }

    {
        let query = bookmarks::SearchQuery {
            id: Some(0),
            tags: Some(vec!["all".to_string()]),
            ..Default::default()
        };

        assert_eq!(app.search(query).unwrap().len(), 1);
    }

    {
        let query = bookmarks::SearchQuery {
            id: Some(0),
            tags: Some(vec!["tag5".to_string()]),
            ..Default::default()
        };

        assert_eq!(app.search(query).unwrap().len(), 0);
    }

    {
        let query = bookmarks::SearchQuery {
            id: Some(0),
            tags: Some(vec!["tag0".to_string()]),
            ..Default::default()
        };

        assert_eq!(app.search(query).unwrap().len(), 1);
    }

    {
        let query = bookmarks::SearchQuery {
            tags: Some(vec!["all".to_string()]),
            ..Default::default()
        };

        assert_eq!(app.search(query).unwrap().len(), 10);
    }

    {
        let query = bookmarks::SearchQuery {
            tags: Some(vec!["all".to_string(), "tag5".to_string()]),
            ..Default::default()
        };

        assert_eq!(app.search(query).unwrap().len(), 1);
    }
}

#[test]
pub fn test_bookmark_update() {
    let app = create_app();

    let opts = crate::app::AddOpts {
        no_https_upgrade: false,
        async_meta: false,
        meta_opts: None,
        skip_rules: false,
    };

    for b in 0..10 {
        let bmark_create = bookmarks::BookmarkCreate {
            url: format!("https://example.com/{b}"),
            title: Some(format!("very cool title #{b}")),
            tags: Some(vec![format!("all"), format!("tag{b}")]),
            ..Default::default()
        };

        app.create(bmark_create.clone(), opts.clone()).unwrap();
    }

    let bmark_update = bookmarks::BookmarkUpdate {
        title: Some("yeah".to_string()),
        description: Some("1".to_string()),
        tags: Some(vec!["heh".to_string()]),
        ..Default::default()
    };

    app.update(0, bmark_update.clone()).unwrap();
    app.update(1, bmark_update).unwrap();

    {
        let query = bookmarks::SearchQuery {
            title: Some("yeah".to_string()),
            ..Default::default()
        };

        assert_eq!(app.search(query).unwrap().len(), 2);
    }
}

#[test]
pub fn test_bookmark_dedup() {
    let app = create_app();
    let bmark_create = bookmarks::BookmarkCreate {
        url: "https://example.com/".to_string(),
        ..Default::default()
    };

    let opts = crate::app::AddOpts {
        no_https_upgrade: false,
        async_meta: false,
        meta_opts: None,
        skip_rules: false,
    };

    let _ = app.create(bmark_create.clone(), opts.clone()).unwrap();

    assert!(app.create(bmark_create.clone(), opts.clone()).is_err());

    app.config().write().unwrap().allow_duplicates = true;

    assert!(app.create(bmark_create, opts).is_ok());
}
