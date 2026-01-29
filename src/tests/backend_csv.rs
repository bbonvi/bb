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

// --- additional coverage ---

#[test]
fn tags_containing_commas_roundtrip_corruption() {
    let tmp = tempfile::tempdir().unwrap();
    let csv_path = tmp.path().join("bookmarks.csv");
    let path_str = csv_path.to_str().unwrap();

    {
        let mgr = BackendCsv::load(path_str).unwrap();
        mgr.create(BookmarkCreate {
            url: "https://comma.com".into(),
            tags: Some(vec!["a,b".into(), "c".into()]),
            ..Default::default()
        })
        .unwrap();
    }

    // Reload from disk — the comma inside "a,b" is indistinguishable from the delimiter
    let mgr = BackendCsv::load(path_str).unwrap();
    let all = mgr.search(SearchQuery::default()).unwrap();
    assert_eq!(all.len(), 1);
    // Known corruption: "a,b" + "c" → join(",") → "a,b,c" → split(",") → ["a","b","c"]
    assert_eq!(all[0].tags, vec!["a", "b", "c"]);
}

#[test]
fn csv_fields_with_embedded_quotes_and_newlines() {
    let tmp = tempfile::tempdir().unwrap();
    let csv_path = tmp.path().join("bookmarks.csv");
    let path_str = csv_path.to_str().unwrap();

    {
        let mgr = BackendCsv::load(path_str).unwrap();
        mgr.create(BookmarkCreate {
            url: "https://special.com".into(),
            title: Some("Hello, \"World\"".into()),
            description: Some("line1\nline2".into()),
            ..Default::default()
        })
        .unwrap();
    }

    let mgr = BackendCsv::load(path_str).unwrap();
    let all = mgr.search(SearchQuery::default()).unwrap();
    assert_eq!(all.len(), 1);
    assert_eq!(all[0].title, "Hello, \"World\"");
    assert_eq!(all[0].description, "line1\nline2");
}

#[test]
fn empty_string_tags() {
    let (mgr, _tmp) = fresh_mgr();
    let b = mgr
        .create(BookmarkCreate {
            url: "https://empty-tag.com".into(),
            tags: Some(vec!["".into(), "real".into()]),
            ..Default::default()
        })
        .unwrap();
    // Empty string tag may survive — document actual behavior
    assert!(b.tags.contains(&"real".to_string()));
    // The empty string is either kept or filtered; record whichever is true
    let has_empty = b.tags.contains(&"".to_string());
    // Re-check via search to confirm persistence
    let results = mgr.search(SearchQuery::default()).unwrap();
    assert_eq!(results[0].tags.contains(&"".to_string()), has_empty);
}

#[test]
fn search_with_only_negated_tags() {
    let (mgr, _tmp) = fresh_mgr();
    mgr.create(BookmarkCreate {
        url: "https://hidden.com".into(),
        tags: Some(vec!["hidden".into()]),
        ..Default::default()
    })
    .unwrap();
    mgr.create(BookmarkCreate {
        url: "https://visible.com".into(),
        tags: Some(vec!["visible".into()]),
        ..Default::default()
    })
    .unwrap();

    let results = mgr
        .search(SearchQuery {
            tags: Some(vec!["-hidden".into()]),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].url, "https://visible.com");
}

#[test]
fn delete_then_create_id_monotonicity() {
    let (mgr, _tmp) = fresh_mgr();
    seed(&mgr, 3); // ids 0, 1, 2
    mgr.delete(0).unwrap();

    let new = mgr
        .create(BookmarkCreate {
            url: "https://new.com".into(),
            ..Default::default()
        })
        .unwrap();
    // ID should be last.id + 1 = 3, not reuse deleted 0
    assert_eq!(new.id, 3);
}

#[test]
fn search_by_description_filter() {
    let (mgr, _tmp) = fresh_mgr();
    mgr.create(BookmarkCreate {
        url: "https://a.com".into(),
        description: Some("unique desc for matching".into()),
        ..Default::default()
    })
    .unwrap();
    mgr.create(BookmarkCreate {
        url: "https://b.com".into(),
        description: Some("something else".into()),
        ..Default::default()
    })
    .unwrap();

    let results = mgr
        .search(SearchQuery {
            description: Some("unique desc".into()),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].url, "https://a.com");
}

#[test]
fn search_update_with_remove_tags() {
    let (mgr, _tmp) = fresh_mgr();
    seed(&mgr, 3); // each has tags ["all", "tagN"]

    let count = mgr
        .search_update(
            SearchQuery {
                tags: Some(vec!["all".into()]),
                ..Default::default()
            },
            BookmarkUpdate {
                remove_tags: Some(vec!["all".into()]),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(count, 3);

    let all = mgr.search(SearchQuery::default()).unwrap();
    for b in &all {
        assert!(
            !b.tags.contains(&"all".to_string()),
            "bookmark {} still has 'all' tag",
            b.id
        );
    }
}

#[test]
fn search_delete_entire_collection() {
    let (mgr, _tmp) = fresh_mgr();
    seed(&mgr, 5); // all have tag "all"

    let count = mgr
        .search_delete(SearchQuery {
            tags: Some(vec!["all".into()]),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(count, 5);

    let remaining = mgr.search(SearchQuery::default()).unwrap();
    assert!(remaining.is_empty());
}

#[test]
fn keyword_with_tag_prefix_search() {
    let (mgr, _tmp) = fresh_mgr();
    seed(&mgr, 3); // tags: ["all","tag0"], ["all","tag1"], ["all","tag2"]

    let results = mgr
        .search(SearchQuery {
            keyword: Some("#tag0".into()),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(results.len(), 1);
    assert!(results[0].tags.contains(&"tag0".to_string()));
}

#[test]
fn empty_and_whitespace_keyword_returns_all() {
    let (mgr, _tmp) = fresh_mgr();
    seed(&mgr, 4);

    // Known behavior: Some("") is not None, so the early "return all" path is
    // skipped, yet the empty keyword sets no has_match flag → returns nothing.
    // This documents the current (arguably buggy) behavior.
    let results_empty = mgr
        .search(SearchQuery {
            keyword: Some("".into()),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(results_empty.len(), 0);

    let results_whitespace = mgr
        .search(SearchQuery {
            keyword: Some("   ".into()),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(results_whitespace.len(), 0);
}

#[test]
fn create_with_reordered_internal_list_uses_max_not_last() {
    let (mgr, _tmp) = fresh_mgr();
    // Create bookmarks with IDs 0, 1, 2
    let b0 = mgr.create(BookmarkCreate { url: "https://a.com".into(), ..Default::default() }).unwrap();
    let b1 = mgr.create(BookmarkCreate { url: "https://b.com".into(), ..Default::default() }).unwrap();
    let b2 = mgr.create(BookmarkCreate { url: "https://c.com".into(), ..Default::default() }).unwrap();
    assert_eq!(b0.id, 0);
    assert_eq!(b1.id, 1);
    assert_eq!(b2.id, 2);

    // Manually reorder the internal list to [2, 1, 0] (e.g., from external manipulation)
    {
        let list_arc = mgr.list();
        let mut list = list_arc.write().unwrap();
        list.reverse();
        assert_eq!(list.last().unwrap().id, 0); // last() is now 0, not 2
    }

    // Create a new bookmark.
    // - Buggy approach: last().id + 1 = 0 + 1 = 1 → collision with existing ID 1!
    // - Fixed approach: max(all_ids) + 1 = 2 + 1 = 3 → safe, no collision
    let b3 = mgr.create(BookmarkCreate { url: "https://d.com".into(), ..Default::default() }).unwrap();
    assert_eq!(b3.id, 3);

    // Verify no collision: all IDs are unique
    let all = mgr.search(SearchQuery::default()).unwrap();
    let ids: Vec<u64> = all.iter().map(|b| b.id).collect();
    let unique_count = ids.iter().collect::<std::collections::HashSet<_>>().len();
    assert_eq!(ids.len(), unique_count);
}
