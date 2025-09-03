use crate::bookmarks;
use crate::bookmarks::BookmarkManager;

#[test]
pub fn test_backend_json_create() {
    let mgr = bookmarks::BackendCsv::load("test-bookmarks.csv")
        .unwrap()
        .wipe_database();

    let created = mgr
        .create(bookmarks::BookmarkCreate {
            title: None,
            description: None,
            tags: None,
            url: "http://example.com".to_string(),
            image_id: None,
            icon_id: None,
        })
        .unwrap();

    let list = mgr.list();
    let list = list.read().unwrap();
    let bmark = list.first().unwrap();
    assert_eq!(&bmark.url, &"http://example.com");
    assert_eq!(&bmark.url, &created.url);
}

#[test]
pub fn test_backend_json_search_by_title() {
    let mgr = bookmarks::BackendCsv::load("test-bookmarks.csv")
        .unwrap()
        .wipe_database();

    mgr.create(bookmarks::BookmarkCreate {
        title: Some("Some kind of TITLE over here".to_string()),
        url: "http://example.com/1".to_string(),
        ..Default::default()
    })
    .unwrap();
    mgr.create(bookmarks::BookmarkCreate {
        title: Some("Another kind of cool title".to_string()),
        url: "http://example.com/2".to_string(),
        ..Default::default()
    })
    .unwrap();

    mgr.create(bookmarks::BookmarkCreate {
        title: Some("lmao".to_string()),
        url: "http://example.com/3".to_string(),
        ..Default::default()
    })
    .unwrap();

    let bmarks = mgr
        .search(bookmarks::SearchQuery {
            title: Some("title".to_string()),
            ..Default::default()
        })
        .unwrap();

    assert_eq!(bmarks.len(), 2);
    assert_eq!(&bmarks.get(0).unwrap().url, &"http://example.com/1");
    assert_eq!(&bmarks.get(1).unwrap().url, &"http://example.com/2");

    mgr.wipe_database();
}

#[test]
pub fn test_backend_json_search_by_description() {
    let mgr = bookmarks::BackendCsv::load("test-bookmarks.csv")
        .unwrap()
        .wipe_database();

    mgr.create(bookmarks::BookmarkCreate {
        title: Some("Some kind of TITLE over here".to_string()),
        description: Some("very cool and meaningful description".to_string()),
        url: "http://example.com/1".to_string(),
        ..Default::default()
    })
    .unwrap();
    mgr.create(bookmarks::BookmarkCreate {
        title: Some("Another kind of cool title".to_string()),
        description: Some("this sucks lol".to_string()),
        url: "http://example.com/2".to_string(),
        ..Default::default()
    })
    .unwrap();

    mgr.create(bookmarks::BookmarkCreate {
        title: Some("lmao".to_string()),
        description: Some("very nicely describing whats going on".to_string()),
        url: "http://example.com/3".to_string(),
        ..Default::default()
    })
    .unwrap();

    let bmarks = mgr
        .search(bookmarks::SearchQuery {
            description: Some("very".to_string()),
            ..Default::default()
        })
        .unwrap();

    assert_eq!(bmarks.len(), 2);
    assert_eq!(&bmarks.get(0).unwrap().url, &"http://example.com/1");
    assert_eq!(&bmarks.get(1).unwrap().url, &"http://example.com/3");

    mgr.wipe_database();
}

#[test]
pub fn test_backend_json_search_by_description_and_title() {
    let mgr = bookmarks::BackendCsv::load("test-bookmarks.csv")
        .unwrap()
        .wipe_database();

    mgr.create(bookmarks::BookmarkCreate {
        title: Some("Some kind of TITLE over here".to_string()),
        description: Some("very cool and meaningful description".to_string()),
        url: "http://example.com/1".to_string(),
        ..Default::default()
    })
    .unwrap();
    mgr.create(bookmarks::BookmarkCreate {
        title: Some("Another kind of cool title".to_string()),
        description: Some("this sucks lol".to_string()),
        url: "http://example.com/2".to_string(),
        ..Default::default()
    })
    .unwrap();

    mgr.create(bookmarks::BookmarkCreate {
        title: Some("lmao".to_string()),
        description: Some("very nicely describing whats going on".to_string()),
        url: "http://example.com/3".to_string(),
        ..Default::default()
    })
    .unwrap();

    let bmarks = mgr
        .search(bookmarks::SearchQuery {
            title: Some("title".to_string()),
            description: Some("very".to_string()),
            ..Default::default()
        })
        .unwrap();

    assert_eq!(bmarks.len(), 1);
    assert_eq!(&bmarks.get(0).unwrap().url, &"http://example.com/1");

    mgr.wipe_database();
}

#[test]
pub fn test_backend_json_search_by_tags() {
    let mgr = bookmarks::BackendCsv::load("test-bookmarks.csv")
        .unwrap()
        .wipe_database();

    mgr.create(bookmarks::BookmarkCreate {
        title: Some("Some kind of TITLE over here".to_string()),
        description: Some("very cool and meaningful description".to_string()),
        url: "http://example.com/1".to_string(),
        tags: Some(vec!["tag1".into(), "tag2".into()]),
        ..Default::default()
    })
    .unwrap();
    mgr.create(bookmarks::BookmarkCreate {
        title: Some("Another kind of cool title".to_string()),
        description: Some("this sucks lol".to_string()),
        url: "http://example.com/2".to_string(),
        tags: Some(vec!["tag2".into(), "tag3".into()]),
        ..Default::default()
    })
    .unwrap();

    mgr.create(bookmarks::BookmarkCreate {
        title: Some("lmao".to_string()),
        description: Some("very nicely describing whats going on".to_string()),
        url: "http://example.com/3".to_string(),
        tags: Some(vec!["tag3".into(), "tag4".into()]),
        ..Default::default()
    })
    .unwrap();

    let bmarks = mgr
        .search(bookmarks::SearchQuery {
            tags: Some(vec!["tag2".into()]),
            ..Default::default()
        })
        .unwrap();

    assert_eq!(bmarks.len(), 2);
    assert_eq!(&bmarks.get(0).unwrap().url, &"http://example.com/1");
    assert_eq!(&bmarks.get(1).unwrap().url, &"http://example.com/2");

    mgr.wipe_database();
}

#[test]
pub fn test_backend_json_search_by_subtags() {
    let mgr = bookmarks::BackendCsv::load("test-bookmarks.csv")
        .unwrap()
        .wipe_database();

    mgr.create(bookmarks::BookmarkCreate {
        url: "http://example.com/1".to_string(),
        tags: Some(vec!["tag/1".into()]),
        ..Default::default()
    })
    .unwrap();

    mgr.create(bookmarks::BookmarkCreate {
        url: "http://example.com/2".to_string(),
        tags: Some(vec!["tag/2".into()]),
        ..Default::default()
    })
    .unwrap();

    mgr.create(bookmarks::BookmarkCreate {
        url: "http://example.com/3".to_string(),
        tags: Some(vec!["tag".into()]),
        ..Default::default()
    })
    .unwrap();

    mgr.create(bookmarks::BookmarkCreate {
        url: "http://example.com/4".to_string(),
        tags: Some(vec!["tags".into()]),
        ..Default::default()
    })
    .unwrap();

    //

    assert_eq!(
        mgr.search(bookmarks::SearchQuery {
            tags: Some(vec!["tags".into()]),
            ..Default::default()
        })
        .unwrap()
        .len(),
        1
    );

    assert_eq!(
        mgr.search(bookmarks::SearchQuery {
            tags: Some(vec!["tag".into()]),
            ..Default::default()
        })
        .unwrap()
        .len(),
        3
    );

    assert_eq!(
        mgr.search(bookmarks::SearchQuery {
            tags: Some(vec!["tag/1".into()]),
            ..Default::default()
        })
        .unwrap()
        .len(),
        1
    );

    assert_eq!(
        &mgr.search(bookmarks::SearchQuery {
            tags: Some(vec!["-tag".into()]),
            ..Default::default()
        })
        .unwrap()
        .first()
        .unwrap()
        .url,
        "http://example.com/4"
    );

    mgr.wipe_database();
}

#[test]
pub fn test_backend_json_delete() {
    let mgr = bookmarks::BackendCsv::load("test-bookmarks.csv")
        .unwrap()
        .wipe_database();

    let created = mgr
        .create(bookmarks::BookmarkCreate {
            title: None,
            description: None,
            tags: None,
            url: "http://example.com".to_string(),
            image_id: None,
            icon_id: None,
        })
        .unwrap();

    {
        let list = mgr.list();
        let list = list.read().unwrap();
        let bmark = list.first().unwrap();
        assert_eq!(&bmark.url, &"http://example.com");
        assert_eq!(&bmark.url, &created.url);
    }

    mgr.delete(0).unwrap().unwrap();
    let list = mgr.list();
    let list = list.read().unwrap();
    assert_eq!(list.len(), 0);
}

#[test]
pub fn test_backend_json_update() {
    let mgr = bookmarks::BackendCsv::load("test-bookmarks.csv")
        .unwrap()
        .wipe_database();

    let _ = mgr
        .create(bookmarks::BookmarkCreate {
            title: Some("lole".to_string()),
            description: None,
            tags: None,
            url: "http://example.com".to_string(),
            image_id: None,
            icon_id: None,
        })
        .unwrap();

    mgr.update(
        0,
        bookmarks::BookmarkUpdate {
            title: Some("yea".to_string()),
            description: Some("what".to_string()),
            ..Default::default()
        },
    )
    .unwrap()
    .unwrap();

    let list = mgr.list();
    let list = list.read().unwrap();
    assert_eq!(list.first().unwrap().title, "yea".to_string());
    assert_eq!(list.first().unwrap().description, "what".to_string());
    assert!(list.first().unwrap().tags.is_empty());
}
