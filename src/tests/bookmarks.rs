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

    mgr.delete(0).unwrap();
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
    .unwrap();

    let list = mgr.list();
    let list = list.read().unwrap();
    assert_eq!(list.first().unwrap().title, "yea".to_string());
    assert_eq!(list.first().unwrap().description, "what".to_string());
    assert!(list.first().unwrap().tags.is_empty());
}

#[test]
pub fn test_fuzzy_search() {
    let mgr = bookmarks::BackendCsv::load("test-bookmarks.csv")
        .unwrap()
        .wipe_database();

    // Create test bookmarks
    mgr.create(bookmarks::BookmarkCreate {
        title: Some("Rust Programming Guide".to_string()),
        description: Some("Learn Rust programming language".to_string()),
        url: "https://rust-lang.org/learn".to_string(),
        tags: Some(vec!["programming".to_string(), "rust".to_string()]),
        ..Default::default()
    })
    .unwrap();

    mgr.create(bookmarks::BookmarkCreate {
        title: Some("Python Tutorial".to_string()),
        description: Some("Python programming tutorial".to_string()),
        url: "https://python.org/tutorial".to_string(),
        tags: Some(vec!["programming".to_string(), "python".to_string()]),
        ..Default::default()
    })
    .unwrap();

    mgr.create(bookmarks::BookmarkCreate {
        title: Some("Web Development".to_string()),
        description: Some("HTML, CSS, JavaScript tutorial".to_string()),
        url: "https://web.dev".to_string(),
        tags: Some(vec!["web".to_string(), "frontend".to_string()]),
        ..Default::default()
    })
    .unwrap();

    // Test fuzzy search by title
    let results = mgr
        .search(bookmarks::SearchQuery {
            fuzzy: Some("rust".to_string()),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].title, "Rust Programming Guide");

    // Test fuzzy search by description
    let results = mgr
        .search(bookmarks::SearchQuery {
            fuzzy: Some("tutorial".to_string()),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(results.len(), 2); // Both Python and Web tutorials
    assert!(results.iter().any(|b| b.title == "Python Tutorial"));
    assert!(results.iter().any(|b| b.title == "Web Development"));

    // Test multi-keyword fuzzy search
    let results = mgr
        .search(bookmarks::SearchQuery {
            fuzzy: Some("python programming".to_string()),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(results.len(), 1); // Only Python has both "python" and "programming"
    assert_eq!(results[0].title, "Python Tutorial");

    // Test multi-keyword with mixed field matches
    let results = mgr
        .search(bookmarks::SearchQuery {
            fuzzy: Some("rust guide".to_string()),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(results.len(), 1); // Rust Programming Guide has both keywords
    assert_eq!(results[0].title, "Rust Programming Guide");

    // Test multi-keyword with tag and text
    let results = mgr
        .search(bookmarks::SearchQuery {
            fuzzy: Some("python #programming".to_string()),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(results.len(), 1); // Python has "python" in title and "programming" tag
    assert_eq!(results[0].title, "Python Tutorial");

    // Test multi-keyword where not all keywords match
    let results = mgr
        .search(bookmarks::SearchQuery {
            fuzzy: Some("python javascript".to_string()),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(results.len(), 0); // No bookmark has both "python" and "javascript"

    // Test fuzzy search by URL
    let results = mgr
        .search(bookmarks::SearchQuery {
            fuzzy: Some("python.org".to_string()),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].title, "Python Tutorial");

    // Test fuzzy search by tag with # prefix
    let results = mgr
        .search(bookmarks::SearchQuery {
            fuzzy: Some("#programming".to_string()),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(results.len(), 2); // Both Rust and Python have programming tag
    assert!(results.iter().any(|b| b.title == "Rust Programming Guide"));
    assert!(results.iter().any(|b| b.title == "Python Tutorial"));

    // Test exact fuzzy search
    let results = mgr
        .search(bookmarks::SearchQuery {
            fuzzy: Some("Rust Programming Guide".to_string()),
            exact: true,
            ..Default::default()
        })
        .unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].title, "Rust Programming Guide");

    // Test fuzzy search with no matches
    let results = mgr
        .search(bookmarks::SearchQuery {
            fuzzy: Some("nonexistent".to_string()),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(results.len(), 0);
}
