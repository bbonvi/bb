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
