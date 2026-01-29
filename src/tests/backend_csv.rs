use crate::bookmarks::{self, BackendCsv, BookmarkCreate, BookmarkManager, BookmarkUpdate, SearchQuery};

fn fresh_mgr() -> (BackendCsv, tempfile::TempDir) {
    let tmp = tempfile::tempdir().expect("failed to create temp dir");
    let csv_path = tmp.path().join("bookmarks.csv");
    let mgr = BackendCsv::load(csv_path.to_str().unwrap()).unwrap();
    (mgr, tmp)
}

fn seed(mgr: &BackendCsv, count: usize) {
    for i in 0..count {
        mgr.create(BookmarkCreate {
            url: format!("https://example.com/{i}"),
            title: Some(format!("Title {i}")),
            description: Some(format!("Description {i}")),
            tags: Some(vec!["all".to_string(), format!("tag{i}")]),
            ..Default::default()
        })
        .unwrap();
    }
}

// --- save / load roundtrip ---

#[test]
fn save_load_roundtrip_preserves_data() {
    let tmp = tempfile::tempdir().unwrap();
    let csv_path = tmp.path().join("bookmarks.csv");
    let path_str = csv_path.to_str().unwrap();

    {
        let mgr = BackendCsv::load(path_str).unwrap();
        mgr.create(BookmarkCreate {
            url: "https://a.com".into(),
            title: Some("A".into()),
            description: Some("desc A".into()),
            tags: Some(vec!["t1".into(), "t2".into()]),
            image_id: Some("img1".into()),
            icon_id: Some("ico1".into()),
        })
        .unwrap();
        mgr.create(BookmarkCreate {
            url: "https://b.com".into(),
            title: Some("B".into()),
            ..Default::default()
        })
        .unwrap();
    }

    // reload from disk
    let mgr = BackendCsv::load(path_str).unwrap();
    let all = mgr.search(SearchQuery::default()).unwrap();
    assert_eq!(all.len(), 2);

    let a = &all[0];
    assert_eq!(a.url, "https://a.com");
    assert_eq!(a.title, "A");
    assert_eq!(a.description, "desc A");
    assert_eq!(a.tags, vec!["t1", "t2"]);
    assert_eq!(a.image_id.as_deref(), Some("img1"));
    assert_eq!(a.icon_id.as_deref(), Some("ico1"));

    let b = &all[1];
    assert_eq!(b.url, "https://b.com");
    assert_eq!(b.title, "B");
    assert!(b.image_id.is_none());
}

#[test]
fn load_nonexistent_creates_empty_csv() {
    let tmp = tempfile::tempdir().unwrap();
    let csv_path = tmp.path().join("new.csv");
    let mgr = BackendCsv::load(csv_path.to_str().unwrap()).unwrap();
    let all = mgr.search(SearchQuery::default()).unwrap();
    assert_eq!(all.len(), 0);
    assert!(csv_path.exists());
}

// --- version counter ---

#[test]
fn version_increments_on_save() {
    let (mgr, _tmp) = fresh_mgr();
    assert_eq!(mgr.version(), 0);

    mgr.create(BookmarkCreate {
        url: "https://a.com".into(),
        ..Default::default()
    })
    .unwrap();
    assert_eq!(mgr.version(), 1);

    mgr.create(BookmarkCreate {
        url: "https://b.com".into(),
        ..Default::default()
    })
    .unwrap();
    assert_eq!(mgr.version(), 2);

    mgr.update(
        0,
        BookmarkUpdate {
            title: Some("X".into()),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(mgr.version(), 3);

    mgr.delete(0).unwrap();
    assert_eq!(mgr.version(), 4);
}

// --- create ---

#[test]
fn create_assigns_sequential_ids() {
    let (mgr, _tmp) = fresh_mgr();
    let b0 = mgr.create(BookmarkCreate { url: "https://a.com".into(), ..Default::default() }).unwrap();
    let b1 = mgr.create(BookmarkCreate { url: "https://b.com".into(), ..Default::default() }).unwrap();
    assert_eq!(b0.id, 0);
    assert_eq!(b1.id, 1);
}

#[test]
fn create_deduplicates_tags() {
    let (mgr, _tmp) = fresh_mgr();
    let b = mgr.create(BookmarkCreate {
        url: "https://a.com".into(),
        tags: Some(vec!["a".into(), "b".into(), "a".into(), "c".into(), "b".into()]),
        ..Default::default()
    }).unwrap();
    assert_eq!(b.tags, vec!["a", "b", "c"]);
}

#[test]
fn create_defaults_empty_fields() {
    let (mgr, _tmp) = fresh_mgr();
    let b = mgr.create(BookmarkCreate {
        url: "https://a.com".into(),
        ..Default::default()
    }).unwrap();
    assert_eq!(b.title, "");
    assert_eq!(b.description, "");
    assert!(b.tags.is_empty());
    assert!(b.image_id.is_none());
    assert!(b.icon_id.is_none());
}

// --- delete ---

#[test]
fn delete_removes_bookmark() {
    let (mgr, _tmp) = fresh_mgr();
    seed(&mgr, 3);
    mgr.delete(1).unwrap();
    let all = mgr.search(SearchQuery::default()).unwrap();
    assert_eq!(all.len(), 2);
    assert!(all.iter().all(|b| b.id != 1));
}

#[test]
fn delete_nonexistent_is_noop() {
    let (mgr, _tmp) = fresh_mgr();
    seed(&mgr, 2);
    mgr.delete(999).unwrap(); // no error
    assert_eq!(mgr.search(SearchQuery::default()).unwrap().len(), 2);
}

// --- update ---

#[test]
fn update_modifies_fields_selectively() {
    let (mgr, _tmp) = fresh_mgr();
    seed(&mgr, 1);

    let updated = mgr.update(0, BookmarkUpdate {
        title: Some("New Title".into()),
        ..Default::default()
    }).unwrap();

    assert_eq!(updated.title, "New Title");
    assert_eq!(updated.description, "Description 0"); // unchanged
    assert_eq!(updated.tags, vec!["all", "tag0"]); // unchanged
}

#[test]
fn update_nonexistent_returns_error() {
    let (mgr, _tmp) = fresh_mgr();
    let result = mgr.update(999, BookmarkUpdate {
        title: Some("X".into()),
        ..Default::default()
    });
    assert!(result.is_err());
}

#[test]
fn update_append_tags() {
    let (mgr, _tmp) = fresh_mgr();
    mgr.create(BookmarkCreate {
        url: "https://a.com".into(),
        tags: Some(vec!["original".into()]),
        ..Default::default()
    }).unwrap();

    let updated = mgr.update(0, BookmarkUpdate {
        append_tags: Some(vec!["new1".into(), "new2".into()]),
        ..Default::default()
    }).unwrap();
    assert_eq!(updated.tags, vec!["original", "new1", "new2"]);
}

#[test]
fn update_append_tags_deduplicates() {
    let (mgr, _tmp) = fresh_mgr();
    mgr.create(BookmarkCreate {
        url: "https://a.com".into(),
        tags: Some(vec!["a".into(), "b".into()]),
        ..Default::default()
    }).unwrap();

    let updated = mgr.update(0, BookmarkUpdate {
        append_tags: Some(vec!["b".into(), "c".into()]),
        ..Default::default()
    }).unwrap();
    assert_eq!(updated.tags, vec!["a", "b", "c"]);
}

#[test]
fn update_remove_tags() {
    let (mgr, _tmp) = fresh_mgr();
    mgr.create(BookmarkCreate {
        url: "https://a.com".into(),
        tags: Some(vec!["a".into(), "b".into(), "c".into()]),
        ..Default::default()
    }).unwrap();

    let updated = mgr.update(0, BookmarkUpdate {
        remove_tags: Some(vec!["b".into()]),
        ..Default::default()
    }).unwrap();
    assert_eq!(updated.tags, vec!["a", "c"]);
}

#[test]
fn update_replace_tags_deduplicates() {
    let (mgr, _tmp) = fresh_mgr();
    mgr.create(BookmarkCreate {
        url: "https://a.com".into(),
        tags: Some(vec!["old".into()]),
        ..Default::default()
    }).unwrap();

    let updated = mgr.update(0, BookmarkUpdate {
        tags: Some(vec!["x".into(), "y".into(), "x".into()]),
        ..Default::default()
    }).unwrap();
    assert_eq!(updated.tags, vec!["x", "y"]);
}

#[test]
fn update_url() {
    let (mgr, _tmp) = fresh_mgr();
    mgr.create(BookmarkCreate { url: "https://old.com".into(), ..Default::default() }).unwrap();
    let updated = mgr.update(0, BookmarkUpdate {
        url: Some("https://new.com".into()),
        ..Default::default()
    }).unwrap();
    assert_eq!(updated.url, "https://new.com");
}

// --- search ---

#[test]
fn search_empty_query_returns_all() {
    let (mgr, _tmp) = fresh_mgr();
    seed(&mgr, 5);
    let results = mgr.search(SearchQuery::default()).unwrap();
    assert_eq!(results.len(), 5);
}

#[test]
fn search_by_id() {
    let (mgr, _tmp) = fresh_mgr();
    seed(&mgr, 5);
    let results = mgr.search(SearchQuery { id: Some(2), ..Default::default() }).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].id, 2);
}

#[test]
fn search_by_id_not_found() {
    let (mgr, _tmp) = fresh_mgr();
    seed(&mgr, 3);
    let results = mgr.search(SearchQuery { id: Some(99), ..Default::default() }).unwrap();
    assert_eq!(results.len(), 0);
}

#[test]
fn search_by_title_substring() {
    let (mgr, _tmp) = fresh_mgr();
    seed(&mgr, 5);
    let results = mgr.search(SearchQuery {
        title: Some("Title 3".into()),
        ..Default::default()
    }).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].id, 3);
}

#[test]
fn search_by_title_case_insensitive() {
    let (mgr, _tmp) = fresh_mgr();
    seed(&mgr, 3);
    let results = mgr.search(SearchQuery {
        title: Some("TITLE 1".into()),
        ..Default::default()
    }).unwrap();
    assert_eq!(results.len(), 1);
}

#[test]
fn search_by_url_exact() {
    let (mgr, _tmp) = fresh_mgr();
    seed(&mgr, 3);
    let results = mgr.search(SearchQuery {
        url: Some("https://example.com/1".into()),
        exact: true,
        ..Default::default()
    }).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].id, 1);
}

#[test]
fn search_by_url_substring() {
    let (mgr, _tmp) = fresh_mgr();
    seed(&mgr, 3);
    let results = mgr.search(SearchQuery {
        url: Some("example.com".into()),
        ..Default::default()
    }).unwrap();
    assert_eq!(results.len(), 3);
}

#[test]
fn search_limit() {
    let (mgr, _tmp) = fresh_mgr();
    seed(&mgr, 10);
    let results = mgr.search(SearchQuery {
        url: Some("example.com".into()),
        limit: Some(3),
        ..Default::default()
    }).unwrap();
    assert_eq!(results.len(), 3);
}

#[test]
fn search_hierarchical_tag_matching() {
    let (mgr, _tmp) = fresh_mgr();
    mgr.create(BookmarkCreate {
        url: "https://a.com".into(),
        tags: Some(vec!["dev/rust".into(), "lang".into()]),
        ..Default::default()
    }).unwrap();
    mgr.create(BookmarkCreate {
        url: "https://b.com".into(),
        tags: Some(vec!["dev/python".into()]),
        ..Default::default()
    }).unwrap();
    mgr.create(BookmarkCreate {
        url: "https://c.com".into(),
        tags: Some(vec!["design".into()]),
        ..Default::default()
    }).unwrap();

    // "dev" matches "dev/rust" and "dev/python" via hierarchical matching
    let results = mgr.search(SearchQuery {
        tags: Some(vec!["dev".into()]),
        ..Default::default()
    }).unwrap();
    assert_eq!(results.len(), 2);
}

#[test]
fn search_negated_tags() {
    let (mgr, _tmp) = fresh_mgr();
    mgr.create(BookmarkCreate {
        url: "https://a.com".into(),
        tags: Some(vec!["keep".into(), "good".into()]),
        ..Default::default()
    }).unwrap();
    mgr.create(BookmarkCreate {
        url: "https://b.com".into(),
        tags: Some(vec!["keep".into(), "bad".into()]),
        ..Default::default()
    }).unwrap();

    // keep tag but exclude "bad"
    let results = mgr.search(SearchQuery {
        tags: Some(vec!["keep".into(), "-bad".into()]),
        ..Default::default()
    }).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].url, "https://a.com");
}

#[test]
fn search_multiple_filters_are_and() {
    let (mgr, _tmp) = fresh_mgr();
    mgr.create(BookmarkCreate {
        url: "https://target.com/page".into(),
        title: Some("Right Title".into()),
        ..Default::default()
    }).unwrap();
    mgr.create(BookmarkCreate {
        url: "https://other.com".into(),
        title: Some("Right Title".into()),
        ..Default::default()
    }).unwrap();

    let results = mgr.search(SearchQuery {
        url: Some("target.com".into()),
        title: Some("right".into()),
        ..Default::default()
    }).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].url, "https://target.com/page");
}

// --- search_delete ---

#[test]
fn search_delete_removes_matching() {
    let (mgr, _tmp) = fresh_mgr();
    seed(&mgr, 5);
    let count = mgr.search_delete(SearchQuery {
        tags: Some(vec!["tag2".into()]),
        ..Default::default()
    }).unwrap();
    assert_eq!(count, 1);
    assert_eq!(mgr.search(SearchQuery::default()).unwrap().len(), 4);
}

#[test]
fn search_delete_no_match_removes_nothing() {
    let (mgr, _tmp) = fresh_mgr();
    seed(&mgr, 3);
    let count = mgr.search_delete(SearchQuery {
        title: Some("nonexistent".into()),
        ..Default::default()
    }).unwrap();
    assert_eq!(count, 0);
    assert_eq!(mgr.search(SearchQuery::default()).unwrap().len(), 3);
}

// --- search_update ---

#[test]
fn search_update_modifies_matching() {
    let (mgr, _tmp) = fresh_mgr();
    seed(&mgr, 5);
    let count = mgr.search_update(
        SearchQuery {
            tags: Some(vec!["all".into()]),
            limit: Some(2),
            ..Default::default()
        },
        BookmarkUpdate {
            description: Some("bulk updated".into()),
            ..Default::default()
        },
    ).unwrap();
    assert_eq!(count, 2);

    let all = mgr.search(SearchQuery::default()).unwrap();
    let updated: Vec<_> = all.iter().filter(|b| b.description == "bulk updated").collect();
    assert_eq!(updated.len(), 2);
}

#[test]
fn search_update_append_tags_across_multiple() {
    let (mgr, _tmp) = fresh_mgr();
    seed(&mgr, 3);
    mgr.search_update(
        SearchQuery {
            tags: Some(vec!["all".into()]),
            ..Default::default()
        },
        BookmarkUpdate {
            append_tags: Some(vec!["new_tag".into()]),
            ..Default::default()
        },
    ).unwrap();

    let all = mgr.search(SearchQuery::default()).unwrap();
    assert!(all.iter().all(|b| b.tags.contains(&"new_tag".to_string())));
}
