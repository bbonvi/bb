use crate::bookmarks;
use crate::bookmarks::BookmarkManager;

#[test]
pub fn test_keyword_search() {
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let csv_path = tmp.path().join("bookmarks.csv");
    let mgr = bookmarks::BackendCsv::load(csv_path.to_str().unwrap()).unwrap();

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

    // Test keyword search by title
    let results = mgr
        .search(bookmarks::SearchQuery {
            keyword: Some("rust".to_string()),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].title, "Rust Programming Guide");

    // Test keyword search by description
    let results = mgr
        .search(bookmarks::SearchQuery {
            keyword: Some("tutorial".to_string()),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(results.len(), 2); // Both Python and Web tutorials
    assert!(results.iter().any(|b| b.title == "Python Tutorial"));
    assert!(results.iter().any(|b| b.title == "Web Development"));

    // Test multi-keyword search
    let results = mgr
        .search(bookmarks::SearchQuery {
            keyword: Some("python programming".to_string()),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(results.len(), 1); // Only Python has both "python" and "programming"
    assert_eq!(results[0].title, "Python Tutorial");

    // Test multi-keyword with mixed field matches
    let results = mgr
        .search(bookmarks::SearchQuery {
            keyword: Some("rust guide".to_string()),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].title, "Rust Programming Guide");

    // Test multi-keyword with tag and text
    let results = mgr
        .search(bookmarks::SearchQuery {
            keyword: Some("python programming".to_string()),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].title, "Python Tutorial");

    // Test multi-keyword where not all keywords match
    let results = mgr
        .search(bookmarks::SearchQuery {
            keyword: Some("python javascript".to_string()),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(results.len(), 0);

    // Test keyword search by URL
    let results = mgr
        .search(bookmarks::SearchQuery {
            keyword: Some("python.org".to_string()),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].title, "Python Tutorial");

    // Test keyword search by tag
    let results = mgr
        .search(bookmarks::SearchQuery {
            keyword: Some("programming".to_string()),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(results.len(), 2);
    assert!(results.iter().any(|b| b.title == "Rust Programming Guide"));
    assert!(results.iter().any(|b| b.title == "Python Tutorial"));

    // Test exact keyword search
    let results = mgr
        .search(bookmarks::SearchQuery {
            keyword: Some("Rust Programming Guide".to_string()),
            exact: true,
            ..Default::default()
        })
        .unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].title, "Rust Programming Guide");

    // Test keyword search with no matches
    let results = mgr
        .search(bookmarks::SearchQuery {
            keyword: Some("nonexistent".to_string()),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(results.len(), 0);
}
