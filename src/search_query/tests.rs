use crate::bookmarks::Bookmark;
use super::{parse, eval, matches};
use super::parser::{SearchFilter, FieldTarget};

fn make_bookmark(title: &str, desc: &str, url: &str, tags: &[&str]) -> Bookmark {
    Bookmark {
        id: 1,
        title: title.to_string(),
        description: desc.to_string(),
        url: url.to_string(),
        tags: tags.iter().map(|s| s.to_string()).collect(),
        image_id: None,
        icon_id: None,
    }
}

// === Tolerant parsing — new tests ===

#[test]
fn test_empty_returns_none() {
    assert_eq!(parse("").unwrap(), None);
    assert_eq!(parse("   ").unwrap(), None);
}

#[test]
fn test_empty_parens_returns_none() {
    assert_eq!(parse("()").unwrap(), None);
    assert_eq!(parse("( )").unwrap(), None);
}

#[test]
fn test_tag_with_empty_parens() {
    let f = parse("(#dev) ( )").unwrap().unwrap();
    assert_eq!(f, SearchFilter::Term(FieldTarget::Tag, "dev".into()));
}

#[test]
fn test_trailing_and() {
    let f = parse("#dev and").unwrap().unwrap();
    assert_eq!(f, SearchFilter::Term(FieldTarget::Tag, "dev".into()));
}

#[test]
fn test_leading_and() {
    let f = parse("and #dev").unwrap().unwrap();
    assert_eq!(f, SearchFilter::Term(FieldTarget::Tag, "dev".into()));
}

#[test]
fn test_only_and() {
    assert_eq!(parse("and").unwrap(), None);
}

#[test]
fn test_only_not() {
    assert_eq!(parse("not").unwrap(), None);
}

#[test]
fn test_unmatched_lparen() {
    assert_eq!(parse("(").unwrap(), None);
}

#[test]
fn test_unmatched_rparen() {
    assert_eq!(parse(")").unwrap(), None);
}

#[test]
fn test_unterminated_quote_tolerant() {
    let f = parse("\"hello").unwrap().unwrap();
    assert_eq!(f, SearchFilter::Term(FieldTarget::All, "hello".into()));
}

#[test]
fn test_bare_hash() {
    assert_eq!(parse("#").unwrap(), None);
}

#[test]
fn test_collapsed_adjacent_operators() {
    let f = parse("#dev and and #foo").unwrap().unwrap();
    assert_eq!(
        f,
        SearchFilter::And(
            Box::new(SearchFilter::Term(FieldTarget::Tag, "dev".into())),
            Box::new(SearchFilter::Term(FieldTarget::Tag, "foo".into())),
        )
    );
}

#[test]
fn test_workspace_merge_empty_keyword() {
    // Simulates "(#dev) ( )" — the workspace merge scenario
    let f = parse("(#dev) ()").unwrap().unwrap();
    assert_eq!(f, SearchFilter::Term(FieldTarget::Tag, "dev".into()));
}

// === Regression: all original tests ===

#[test]
fn test_parse_simple_word() {
    let f = parse("video").unwrap().unwrap();
    let bm = make_bookmark("My Video", "", "", &[]);
    assert!(eval(&f, &bm));
}

#[test]
fn test_tag_prefix_exact() {
    let bm = make_bookmark("", "", "", &["video"]);
    assert!(matches("#video", &bm).unwrap());
    assert!(!matches("#vid", &bm).unwrap());
}

#[test]
fn test_tag_prefix_hierarchical() {
    let bm = make_bookmark("", "", "", &["programming/rust"]);
    assert!(matches("#programming", &bm).unwrap());
    assert!(!matches("#rust", &bm).unwrap());
}

#[test]
fn test_title_prefix() {
    let bm = make_bookmark("YouTube Tutorial", "", "", &[]);
    assert!(matches(".youtube", &bm).unwrap());
    assert!(matches(".tutorial", &bm).unwrap());
    assert!(!matches(".vimeo", &bm).unwrap());
}

#[test]
fn test_description_prefix() {
    let bm = make_bookmark("", "Learn Rust programming", "", &[]);
    assert!(matches(">rust", &bm).unwrap());
    assert!(!matches(">python", &bm).unwrap());
}

#[test]
fn test_url_prefix() {
    let bm = make_bookmark("", "", "https://github.com/foo", &[]);
    assert!(matches(":github.com", &bm).unwrap());
    assert!(!matches(":gitlab.com", &bm).unwrap());
}

#[test]
fn test_all_fields_no_prefix() {
    let bm = make_bookmark("Rust Guide", "Learn programming", "https://rust-lang.org", &["dev"]);
    assert!(matches("rust", &bm).unwrap());
    assert!(matches("programming", &bm).unwrap());
    assert!(matches("rust-lang", &bm).unwrap());
    assert!(matches("dev", &bm).unwrap());
}

#[test]
fn test_quoted_phrase_title() {
    let bm = make_bookmark("A YouTube Video", "", "", &[]);
    assert!(matches(".\"youtube video\"", &bm).unwrap());
    assert!(!matches(".\"video youtube\"", &bm).unwrap());
}

#[test]
fn test_quoted_phrase_all_fields() {
    let bm = make_bookmark("Rust Programming", "", "", &[]);
    assert!(matches("\"rust programming\"", &bm).unwrap());
    assert!(!matches("\"programming rust\"", &bm).unwrap());
}

#[test]
fn test_and_explicit() {
    let bm = make_bookmark("Rust Video", "", "", &[]);
    assert!(matches("rust and video", &bm).unwrap());
    assert!(!matches("rust and python", &bm).unwrap());
}

#[test]
fn test_implicit_and() {
    let bm = make_bookmark("Rust Video", "", "", &[]);
    assert!(matches("rust video", &bm).unwrap());
    assert!(!matches("rust python", &bm).unwrap());
}

#[test]
fn test_or() {
    let bm = make_bookmark("Rust Guide", "", "", &[]);
    assert!(matches("rust or python", &bm).unwrap());
    assert!(matches("python or rust", &bm).unwrap());
    assert!(!matches("python or java", &bm).unwrap());
}

#[test]
fn test_not() {
    let bm = make_bookmark("Rust Guide", "", "", &["programming"]);
    assert!(matches("not #archived", &bm).unwrap());
    assert!(!matches("not #programming", &bm).unwrap());
}

#[test]
fn test_precedence_not_over_and() {
    let bm = make_bookmark("Rust Guide", "", "", &["programming"]);
    assert!(matches("not #archived and rust", &bm).unwrap());
}

#[test]
fn test_precedence_and_over_or() {
    let bm = make_bookmark("", "", "", &["audio"]);
    assert!(matches("#video and .youtube or #audio", &bm).unwrap());
}

#[test]
fn test_parentheses_grouping() {
    let bm = make_bookmark("Spotify Podcast", "", "", &["audio"]);
    assert!(matches("(#video and .spotify) or #audio", &bm).unwrap());
    assert!(!matches("#video and (.spotify or #audio)", &bm).unwrap());
}

#[test]
fn test_deeply_nested_parentheses() {
    let bm = make_bookmark("Scala Akka Streams Guide", "", "https://doc.akka.io", &["jvm", "reactive"]);
    assert!(matches("(#jvm and (#reactive and (.akka or (:lightbend)))) or #python", &bm).unwrap());
    assert!(!matches("(#jvm and (#reactive and (.spring or (:lightbend)))) or #python", &bm).unwrap());
    let bm2 = make_bookmark("Django REST", "", "", &["python"]);
    assert!(matches("(#jvm and (#reactive and (.akka or (:lightbend)))) or #python", &bm2).unwrap());
}

#[test]
fn test_escape_hash() {
    let bm = make_bookmark("#hashtag title", "", "", &[]);
    assert!(matches("\\#hashtag", &bm).unwrap());
}

#[test]
fn test_escape_dot() {
    let bm = make_bookmark(".dotfile", "", "", &[]);
    assert!(matches("\\.dotfile", &bm).unwrap());
}

#[test]
fn test_quoted_reserved_words() {
    let bm = make_bookmark("and or not", "", "", &[]);
    assert!(matches("\"and\"", &bm).unwrap());
    assert!(matches("\"or\"", &bm).unwrap());
    assert!(matches("\"not\"", &bm).unwrap());
}

#[test]
fn test_case_insensitive() {
    let bm = make_bookmark("RUST Guide", "Learn PYTHON", "https://GITHUB.COM", &["Programming"]);
    assert!(matches("rust", &bm).unwrap());
    assert!(matches("python", &bm).unwrap());
    assert!(matches(":github.com", &bm).unwrap());
    assert!(matches("#programming", &bm).unwrap());
}

#[test]
fn test_complex_rfc_example() {
    let bm = make_bookmark("YouTube car video Tutorial", "some desc", "https://youtube.com", &["video"]);
    assert!(!matches("(#video and .youtube and not .\"car video\") or (#youtube and not .\"car video\")", &bm).unwrap());
    let bm2 = make_bookmark("YouTube Tutorial", "car video desc", "https://youtube.com", &["video"]);
    assert!(matches("(#video and .youtube and not .\"car video\") or (#youtube and not .\"car video\")", &bm2).unwrap());
}

#[test]
fn test_not_tag_exclude() {
    let bm = make_bookmark("Old Stuff", "", "", &["archived"]);
    assert!(!matches("not #archived", &bm).unwrap());
    let bm2 = make_bookmark("New Stuff", "", "", &["active"]);
    assert!(matches("not #archived", &bm2).unwrap());
}

#[test]
fn test_tag_prefix_vs_all_field() {
    let bm = make_bookmark("", "tagged video content", "", &[]);
    assert!(!matches("#video", &bm).unwrap());
    assert!(matches("video", &bm).unwrap());
}

#[test]
fn test_double_not() {
    let bm = make_bookmark("Rust", "", "", &[]);
    assert!(matches("not not rust", &bm).unwrap());
}
