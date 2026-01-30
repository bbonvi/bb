use crate::rules;

#[test]
pub fn test_plaintext_matches() {
    // plain text matches
    assert!(rules::Rule::is_string_matches("one two", "1 one two 3"));
    assert!(rules::Rule::is_string_matches("1 one", "1 one"));
    assert!(!rules::Rule::is_string_matches("1 one", "1"));
    assert!(rules::Rule::is_string_matches("", "123"));
    assert!(rules::Rule::is_string_matches("r/testing", "r/testing"));
    assert!(rules::Rule::is_string_matches("/testing/", "/testing/"));
}

#[test]
pub fn test_regex_matches() {
    // regex matches
    assert!(rules::Rule::is_string_matches(
        "r//testing//",
        "example.com/testing/this/string"
    ));
    assert!(rules::Rule::is_string_matches(
        "r/example...m/",
        "example.com/testing/this/string"
    ));
    assert!(rules::Rule::is_string_matches(
        "r/test.*string/",
        "example.com/testing/this/string"
    ));
    assert!(!rules::Rule::is_string_matches(
        "r/naur/",
        "example.com/testing/this/string"
    ));
}

#[test]
pub fn test_record_match() {
    let rule = rules::Rule {
        url: Some("interestingpage.com".to_string()),
        description: None,
        title: None,
        tags: None,
        query: None,
        comment: None,
        action: rules::Action::UpdateBookmark {
            tags: Some(vec!["favorite".to_string()]),
            title: None,
            description: None,
        },
    };

    assert!(rule.is_match(&rules::Record {
        url: "veryinterestingpage.com/lole".into(),
        title: None,
        description: None,
        tags: None,
    }));

    assert!(!rule.is_match(&rules::Record {
        url: "stupidpage.com/ihateit".into(),
        title: None,
        description: None,
        tags: None,
    }));
}

#[test]
pub fn test_rule_matches_by_title() {
    let rule = rules::Rule {
        url: None,
        title: Some("important".to_string()),
        description: None,
        tags: None,
        query: None,
        comment: None,
        action: rules::Action::UpdateBookmark {
            tags: None,
            title: None,
            description: None,
        },
    };

    assert!(rule.is_match(&rules::Record {
        url: "https://any.com".into(),
        title: Some("Very Important Page".into()),
        description: None,
        tags: None,
    }));

    assert!(!rule.is_match(&rules::Record {
        url: "https://any.com".into(),
        title: Some("Boring Page".into()),
        description: None,
        tags: None,
    }));
}

#[test]
pub fn test_rule_matches_by_description() {
    let rule = rules::Rule {
        url: None,
        title: None,
        description: Some("tutorial".to_string()),
        tags: None,
        query: None,
        comment: None,
        action: rules::Action::UpdateBookmark {
            tags: None,
            title: None,
            description: None,
        },
    };

    assert!(rule.is_match(&rules::Record {
        url: "https://any.com".into(),
        title: None,
        description: Some("A great tutorial on Rust".into()),
        tags: None,
    }));

    assert!(!rule.is_match(&rules::Record {
        url: "https://any.com".into(),
        title: None,
        description: Some("Just some article".into()),
        tags: None,
    }));
}

#[test]
pub fn test_rule_matches_by_tags_with_url() {
    let rule = rules::Rule {
        url: Some("any.com".to_string()),
        title: None,
        description: None,
        tags: Some(vec!["programming".to_string(), "rust".to_string()]),
        query: None,
        comment: None,
        action: rules::Action::UpdateBookmark {
            tags: None,
            title: None,
            description: None,
        },
    };

    // has both tags in same order — match
    assert!(rule.is_match(&rules::Record {
        url: "https://any.com".into(),
        title: None,
        description: None,
        tags: Some(vec![
            "programming".to_string(),
            "rust".to_string(),
            "extra".to_string(),
        ]),
    }));

    // only one tag — no match
    assert!(!rule.is_match(&rules::Record {
        url: "https://any.com".into(),
        title: None,
        description: None,
        tags: Some(vec!["programming".to_string()]),
    }));

    // no tags — no match
    assert!(!rule.is_match(&rules::Record {
        url: "https://any.com".into(),
        title: None,
        description: None,
        tags: None,
    }));
}

#[test]
pub fn test_rule_empty_tags_matches_untagged() {
    // Empty tags vec matches records with no tags.
    let rule = rules::Rule {
        url: None,
        title: None,
        description: None,
        tags: Some(vec![]),
        query: None,
        comment: None,
        action: rules::Action::UpdateBookmark {
            tags: None,
            title: None,
            description: None,
        },
    };

    assert!(rule.is_match(&rules::Record {
        url: "https://any.com".into(),
        title: None,
        description: None,
        tags: None,
    }));

    assert!(rule.is_match(&rules::Record {
        url: "https://any.com".into(),
        title: None,
        description: None,
        tags: Some(vec![]),
    }));

    // Non-empty record tags don't match empty rule tags
    assert!(!rule.is_match(&rules::Record {
        url: "https://any.com".into(),
        title: None,
        description: None,
        tags: Some(vec!["has-tag".to_string()]),
    }));
}

#[test]
pub fn test_rule_multiple_fields_all_must_match() {
    let rule = rules::Rule {
        url: Some("github.com".to_string()),
        title: Some("repo".to_string()),
        description: None,
        tags: None,
        query: None,
        comment: None,
        action: rules::Action::UpdateBookmark {
            tags: None,
            title: None,
            description: None,
        },
    };

    // both match
    assert!(rule.is_match(&rules::Record {
        url: "https://github.com/user/repo".into(),
        title: Some("My repo".into()),
        description: None,
        tags: None,
    }));

    // url matches but title doesn't
    assert!(!rule.is_match(&rules::Record {
        url: "https://github.com/user/thing".into(),
        title: Some("My project".into()),
        description: None,
        tags: None,
    }));

    // title matches but url doesn't
    assert!(!rule.is_match(&rules::Record {
        url: "https://gitlab.com/user/repo".into(),
        title: Some("My repo".into()),
        description: None,
        tags: None,
    }));
}

#[test]
pub fn test_rule_tag_matching_is_case_insensitive() {
    let rule = rules::Rule {
        url: Some("any.com".to_string()),
        title: None,
        description: None,
        tags: Some(vec!["Rust".to_string()]),
        query: None,
        comment: None,
        action: rules::Action::UpdateBookmark {
            tags: None,
            title: None,
            description: None,
        },
    };

    assert!(rule.is_match(&rules::Record {
        url: "https://any.com".into(),
        title: None,
        description: None,
        tags: Some(vec!["rust".to_string()]),
    }));
}

#[test]
pub fn test_rule_tags_only_matches() {
    let rule = rules::Rule {
        url: None,
        title: None,
        description: None,
        tags: Some(vec!["rust".to_string()]),
        query: None,
        comment: None,
        action: rules::Action::UpdateBookmark {
            tags: None,
            title: None,
            description: None,
        },
    };

    assert!(rule.is_match(&rules::Record {
        url: "https://any.com".into(),
        title: None,
        description: None,
        tags: Some(vec!["rust".to_string()]),
    }));

    assert!(!rule.is_match(&rules::Record {
        url: "https://any.com".into(),
        title: None,
        description: None,
        tags: Some(vec!["python".to_string()]),
    }));
}

#[test]
pub fn test_rule_tag_order_independent() {
    let rule = rules::Rule {
        url: Some("any.com".to_string()),
        title: None,
        description: None,
        tags: Some(vec!["b".to_string(), "a".to_string()]),
        query: None,
        comment: None,
        action: rules::Action::UpdateBookmark {
            tags: None,
            title: None,
            description: None,
        },
    };

    // any order matches
    assert!(rule.is_match(&rules::Record {
        url: "https://any.com".into(),
        title: None,
        description: None,
        tags: Some(vec!["a".to_string(), "b".to_string()]),
    }));

    assert!(rule.is_match(&rules::Record {
        url: "https://any.com".into(),
        title: None,
        description: None,
        tags: Some(vec!["b".to_string(), "a".to_string()]),
    }));
}

#[test]
#[should_panic(expected = "malformed regex")]
pub fn test_malformed_regex_panics() {
    // is_string_matches panics on malformed regex via .expect()
    rules::Rule::is_string_matches("r/[unclosed/", "input");
}

#[test]
pub fn test_title_rule_vs_none_title_record() {
    // When rule has a title condition but record.title is None,
    // the `if let (Some(..), Some(..))` silently skips the check.
    // The rule still matches if other conditions pass.
    let rule = rules::Rule {
        url: Some("any.com".to_string()),
        title: Some("x".to_string()),
        description: None,
        tags: None,
        query: None,
        comment: None,
        action: rules::Action::UpdateBookmark {
            tags: None,
            title: None,
            description: None,
        },
    };

    assert!(rule.is_match(&rules::Record {
        url: "https://any.com".into(),
        title: None,
        description: None,
        tags: None,
    }));
}

#[test]
pub fn test_description_rule_vs_none_description_record() {
    // Same silent-skip behavior for description: rule has description
    // condition but record.description is None — check is skipped.
    let rule = rules::Rule {
        url: Some("any.com".to_string()),
        title: None,
        description: Some("tutorial".to_string()),
        tags: None,
        query: None,
        comment: None,
        action: rules::Action::UpdateBookmark {
            tags: None,
            title: None,
            description: None,
        },
    };

    assert!(rule.is_match(&rules::Record {
        url: "https://any.com".into(),
        title: None,
        description: None,
        tags: None,
    }));
}

#[test]
pub fn test_no_condition_rule_returns_false() {
    // All condition fields are None — has_any_condition stays false.
    let rule = rules::Rule {
        url: None,
        title: None,
        description: None,
        tags: None,
        query: None,
        comment: None,
        action: rules::Action::UpdateBookmark {
            tags: None,
            title: None,
            description: None,
        },
    };

    assert!(!rule.is_match(&rules::Record {
        url: "https://any.com".into(),
        title: Some("anything".into()),
        description: Some("anything".into()),
        tags: Some(vec!["tag".to_string()]),
    }));
}

#[test]
pub fn test_action_fields_exist() {
    // Verify Action::UpdateBookmark can carry all optional fields.
    let action = rules::Action::UpdateBookmark {
        title: Some("New Title".to_string()),
        description: Some("New Description".to_string()),
        tags: Some(vec!["tag1".to_string(), "tag2".to_string()]),
    };

    match action {
        rules::Action::UpdateBookmark {
            title,
            description,
            tags,
        } => {
            assert_eq!(title.unwrap(), "New Title");
            assert_eq!(description.unwrap(), "New Description");
            assert_eq!(tags.unwrap().len(), 2);
        }
    }
}

#[test]
pub fn test_query_simple_term() {
    let rule = rules::Rule {
        url: None,
        title: None,
        description: None,
        tags: None,
        query: Some("rust".to_string()),
        comment: None,
        action: rules::Action::UpdateBookmark {
            tags: None,
            title: None,
            description: None,
        },
    };

    assert!(rule.is_match(&rules::Record {
        url: "https://example.com".into(),
        title: Some("Learning Rust".into()),
        description: None,
        tags: None,
    }));

    assert!(!rule.is_match(&rules::Record {
        url: "https://example.com".into(),
        title: Some("Learning Python".into()),
        description: None,
        tags: None,
    }));
}

#[test]
pub fn test_query_boolean_ops() {
    let rule = rules::Rule {
        url: None,
        title: None,
        description: None,
        tags: None,
        query: Some("rust and tutorial".to_string()),
        comment: None,
        action: rules::Action::UpdateBookmark {
            tags: None,
            title: None,
            description: None,
        },
    };

    assert!(rule.is_match(&rules::Record {
        url: "https://example.com".into(),
        title: Some("Rust Tutorial".into()),
        description: None,
        tags: None,
    }));

    assert!(!rule.is_match(&rules::Record {
        url: "https://example.com".into(),
        title: Some("Rust Reference".into()),
        description: None,
        tags: None,
    }));
}

#[test]
pub fn test_query_not_operator() {
    let rule = rules::Rule {
        url: None,
        title: None,
        description: None,
        tags: None,
        query: Some("rust not beginner".to_string()),
        comment: None,
        action: rules::Action::UpdateBookmark {
            tags: None,
            title: None,
            description: None,
        },
    };

    assert!(rule.is_match(&rules::Record {
        url: "https://example.com".into(),
        title: Some("Advanced Rust".into()),
        description: None,
        tags: None,
    }));

    assert!(!rule.is_match(&rules::Record {
        url: "https://example.com".into(),
        title: Some("Rust beginner guide".into()),
        description: None,
        tags: None,
    }));
}

#[test]
pub fn test_query_field_prefix_tag() {
    let rule = rules::Rule {
        url: None,
        title: None,
        description: None,
        tags: None,
        query: Some("#programming".to_string()),
        comment: None,
        action: rules::Action::UpdateBookmark {
            tags: None,
            title: None,
            description: None,
        },
    };

    assert!(rule.is_match(&rules::Record {
        url: "https://example.com".into(),
        title: Some("Any".into()),
        description: None,
        tags: Some(vec!["programming".to_string()]),
    }));

    assert!(!rule.is_match(&rules::Record {
        url: "https://example.com".into(),
        title: Some("Any".into()),
        description: None,
        tags: Some(vec!["cooking".to_string()]),
    }));
}

#[test]
pub fn test_query_field_prefix_url() {
    let rule = rules::Rule {
        url: None,
        title: None,
        description: None,
        tags: None,
        query: Some(":github.com".to_string()),
        comment: None,
        action: rules::Action::UpdateBookmark {
            tags: None,
            title: None,
            description: None,
        },
    };

    assert!(rule.is_match(&rules::Record {
        url: "https://github.com/user/repo".into(),
        title: None,
        description: None,
        tags: None,
    }));

    assert!(!rule.is_match(&rules::Record {
        url: "https://gitlab.com/user/repo".into(),
        title: None,
        description: None,
        tags: None,
    }));
}

#[test]
pub fn test_query_combined_with_url_condition() {
    let rule = rules::Rule {
        url: Some("example.com".to_string()),
        title: None,
        description: None,
        tags: None,
        query: Some("#rust".to_string()),
        comment: None,
        action: rules::Action::UpdateBookmark {
            tags: None,
            title: None,
            description: None,
        },
    };

    // Both conditions match
    assert!(rule.is_match(&rules::Record {
        url: "https://example.com/page".into(),
        title: None,
        description: None,
        tags: Some(vec!["rust".to_string()]),
    }));

    // URL matches but query doesn't
    assert!(!rule.is_match(&rules::Record {
        url: "https://example.com/page".into(),
        title: None,
        description: None,
        tags: Some(vec!["python".to_string()]),
    }));

    // Query matches but URL doesn't
    assert!(!rule.is_match(&rules::Record {
        url: "https://other.com/page".into(),
        title: None,
        description: None,
        tags: Some(vec!["rust".to_string()]),
    }));
}

#[test]
pub fn test_query_invalid_returns_false() {
    let rule = rules::Rule {
        url: None,
        title: None,
        description: None,
        tags: None,
        query: Some("(unclosed".to_string()),
        comment: None,
        action: rules::Action::UpdateBookmark {
            tags: None,
            title: None,
            description: None,
        },
    };

    // Invalid query -> no match
    assert!(!rule.is_match(&rules::Record {
        url: "https://example.com".into(),
        title: Some("anything".into()),
        description: None,
        tags: None,
    }));
}

#[test]
pub fn test_query_empty_string_returns_false() {
    let rule = rules::Rule {
        url: None,
        title: None,
        description: None,
        tags: None,
        query: Some("".to_string()),
        comment: None,
        action: rules::Action::UpdateBookmark {
            tags: None,
            title: None,
            description: None,
        },
    };

    // Empty query string -> parse error -> no match
    assert!(!rule.is_match(&rules::Record {
        url: "https://example.com".into(),
        title: Some("anything".into()),
        description: None,
        tags: None,
    }));
}
