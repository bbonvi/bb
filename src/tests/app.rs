use std::sync::mpsc;
use std::sync::Arc;
use std::sync::RwLock;

use crate::app::task_runner::Task;
use crate::app::{backend::AppBackend, local::AppLocal};
use crate::bookmarks;
use crate::storage;

/// Creates an isolated AppLocal using a unique temp directory.
/// Each test gets its own directory so parallel tests never collide,
/// and no real data is touched.
pub fn create_app() -> (AppLocal, tempfile::TempDir) {
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let csv_path = tmp.path().join("bookmarks.csv");
    let config_path = tmp.path().to_str().unwrap().to_string();

    let bmark_mgr = Arc::new(
        bookmarks::BackendCsv::load(csv_path.to_str().unwrap())
            .expect("failed to create bookmark csv"),
    );
    let storage_mgr = Arc::new(storage::BackendLocal::new(
        tmp.path().join("uploads").to_str().unwrap(),
    ).expect("failed to create storage"));

    let (task_tx, _) = mpsc::channel::<Task>();
    let config = Arc::new(RwLock::new(crate::config::Config::load_with(&config_path)
        .expect("failed to load config")));

    let handle = std::thread::spawn(move || {});

    let app = AppLocal::new_with(
        bmark_mgr,
        storage_mgr,
        Arc::new(task_tx),
        Some(handle),
        config,
    );
    (app, tmp)
}

fn default_add_opts() -> crate::app::backend::AddOpts {
    crate::app::backend::AddOpts {
        no_https_upgrade: false,
        async_meta: false,
        meta_opts: None,
        skip_rules: false,
    }
}

#[test]
pub fn test_create_bookmark() {
    let (app, _tmp) = create_app();
    let bmark_create = bookmarks::BookmarkCreate {
        url: "https://example.com/lmao-what".to_string(),
        title: Some("lmao what".to_string()),
        ..Default::default()
    };

    let (bookmark, _report) = app.create(bmark_create, default_add_opts()).unwrap();
    assert_eq!(&bookmark.url, "https://example.com/lmao-what");
}

#[test]
pub fn test_bookmark_create() {
    let (app, _tmp) = create_app();
    let bmark_create = bookmarks::BookmarkCreate {
        url: "https://example.com/2".to_string(),
        title: Some("lmao what2".to_string()),
        ..Default::default()
    };

    let (bookmark, _) = app.create(bmark_create, default_add_opts()).unwrap();
    assert_eq!(&bookmark.url, "https://example.com/2");
    assert_eq!(&bookmark.title, "lmao what2");
}

#[test]
pub fn test_bookmark_search() {
    let (app, _tmp) = create_app();
    let opts = default_add_opts();

    for b in 0..10 {
        let bmark_create = bookmarks::BookmarkCreate {
            url: format!("https://example.com/{b}"),
            title: Some(format!("very cool title #{b}")),
            tags: Some(vec!["all".to_string(), format!("tag{b}")]),
            ..Default::default()
        };
        app.create(bmark_create, opts.clone()).unwrap();
    }

    // search by id
    {
        let query = bookmarks::SearchQuery {
            id: Some(0),
            ..Default::default()
        };
        assert_eq!(app.search(query).unwrap().len(), 1);
    }

    // search by id + tag (intersection)
    {
        let query = bookmarks::SearchQuery {
            id: Some(0),
            tags: Some(vec!["all".to_string()]),
            ..Default::default()
        };
        assert_eq!(app.search(query).unwrap().len(), 1);
    }

    // search by id + non-matching tag
    {
        let query = bookmarks::SearchQuery {
            id: Some(0),
            tags: Some(vec!["tag5".to_string()]),
            ..Default::default()
        };
        assert_eq!(app.search(query).unwrap().len(), 0);
    }

    // search by id + matching tag
    {
        let query = bookmarks::SearchQuery {
            id: Some(0),
            tags: Some(vec!["tag0".to_string()]),
            ..Default::default()
        };
        assert_eq!(app.search(query).unwrap().len(), 1);
    }

    // search by tag only — all 10 bookmarks
    {
        let query = bookmarks::SearchQuery {
            tags: Some(vec!["all".to_string()]),
            ..Default::default()
        };
        assert_eq!(app.search(query).unwrap().len(), 10);
    }

    // search by two tags (AND) — exactly one
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
    let (app, _tmp) = create_app();
    let opts = default_add_opts();

    for b in 0..10 {
        let bmark_create = bookmarks::BookmarkCreate {
            url: format!("https://example.com/{b}"),
            title: Some(format!("very cool title #{b}")),
            tags: Some(vec!["all".to_string(), format!("tag{b}")]),
            ..Default::default()
        };
        app.create(bmark_create, opts.clone()).unwrap();
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
    let (app, _tmp) = create_app();
    let bmark_create = bookmarks::BookmarkCreate {
        url: "https://example.com/".to_string(),
        ..Default::default()
    };
    let opts = default_add_opts();

    // first create succeeds
    app.create(bmark_create.clone(), opts.clone()).unwrap();

    // duplicate URL is rejected
    assert!(app.create(bmark_create.clone(), opts.clone()).is_err());

    // third attempt with same URL is still rejected
    assert!(app.create(bmark_create, opts).is_err());
}

// --- merge_metadata force overwrite ---

#[test]
fn merge_metadata_without_force_skips_existing_fields() {
    let (app, _tmp) = create_app();
    let (bmark, _) = app.create(
        bookmarks::BookmarkCreate {
            url: "https://example.com".into(),
            title: Some("Old Title".into()),
            description: Some("Old Desc".into()),
            ..Default::default()
        },
        default_add_opts(),
    ).unwrap();

    let meta = crate::metadata::Metadata {
        title: Some("New Title".into()),
        description: Some("New Desc".into()),
        ..Default::default()
    };

    let img_config = crate::config::ImageConfig::default();
    let result = AppLocal::merge_metadata(
        bmark.clone(),
        meta,
        app.storage_mgr.clone(),
        app.bmark_mgr.clone(),
        &img_config,
        false,
    ).unwrap();

    assert_eq!(result.title, "Old Title");
    assert_eq!(result.description, "Old Desc");
}

#[test]
fn merge_metadata_with_force_overwrites_existing_fields() {
    let (app, _tmp) = create_app();
    let (bmark, _) = app.create(
        bookmarks::BookmarkCreate {
            url: "https://example.com".into(),
            title: Some("Old Title".into()),
            description: Some("Old Desc".into()),
            ..Default::default()
        },
        default_add_opts(),
    ).unwrap();

    let meta = crate::metadata::Metadata {
        title: Some("New Title".into()),
        description: Some("New Desc".into()),
        ..Default::default()
    };

    let img_config = crate::config::ImageConfig::default();
    let result = AppLocal::merge_metadata(
        bmark.clone(),
        meta,
        app.storage_mgr.clone(),
        app.bmark_mgr.clone(),
        &img_config,
        true,
    ).unwrap();

    assert_eq!(result.title, "New Title");
    assert_eq!(result.description, "New Desc");
}
