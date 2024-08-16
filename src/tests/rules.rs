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
