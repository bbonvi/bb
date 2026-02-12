use crate::bookmarks::Bookmark;
use super::{eval, matches, parse, parse_tolerant, required_id_constraint, RequiredId};
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

// === Strict parse — error cases ===

#[test]
fn test_strict_empty_is_err() {
    assert!(parse("").is_err());
    assert!(parse("   ").is_err());
}

#[test]
fn test_strict_trailing_operator_is_err() {
    assert!(parse("#dev and").is_err());
}

#[test]
fn test_strict_leading_operator_is_err() {
    assert!(parse("or #dev").is_err());
}

#[test]
fn test_strict_unmatched_lparen_is_err() {
    assert!(parse("(foo").is_err());
}

#[test]
fn test_strict_unmatched_rparen_is_err() {
    assert!(parse("foo)").is_err());
}

#[test]
fn test_strict_dangling_double_operator_is_err() {
    // Has real terms but dangling operator between
    assert!(parse("#dev and and #foo").is_err());
}

// === Strict parse — literal treatment (no error) ===

#[test]
fn test_strict_unmatched_quote_literal() {
    // `"hello` → literal search for "hello"
    let f = parse("\"hello").unwrap();
    assert_eq!(f, SearchFilter::Term(FieldTarget::All, "hello".into()));
}

#[test]
fn test_strict_bare_hash_literal() {
    // `#` alone → literal word "#"
    let f = parse("#").unwrap();
    assert_eq!(f, SearchFilter::Term(FieldTarget::All, "#".into()));
}

#[test]
fn test_strict_bare_dot_literal() {
    let f = parse(".").unwrap();
    assert_eq!(f, SearchFilter::Term(FieldTarget::All, ".".into()));
}

#[test]
fn test_strict_bare_gt_literal() {
    let f = parse(">").unwrap();
    assert_eq!(f, SearchFilter::Term(FieldTarget::All, ">".into()));
}

#[test]
fn test_strict_bare_colon_literal() {
    let f = parse(":").unwrap();
    assert_eq!(f, SearchFilter::Term(FieldTarget::All, ":".into()));
}

#[test]
fn test_strict_bare_equals_literal() {
    let f = parse("=").unwrap();
    assert_eq!(f, SearchFilter::Term(FieldTarget::All, "=".into()));
}

#[test]
fn test_strict_only_and_becomes_literal() {
    // `and` alone → literal word "and"
    let f = parse("and").unwrap();
    assert_eq!(f, SearchFilter::Term(FieldTarget::All, "and".into()));
}

#[test]
fn test_strict_or_not_becomes_literal() {
    // `or not` → literal words "or", "not" (implicit AND)
    let f = parse("or not").unwrap();
    assert_eq!(
        f,
        SearchFilter::And(
            Box::new(SearchFilter::Term(FieldTarget::All, "or".into())),
            Box::new(SearchFilter::Term(FieldTarget::All, "not".into())),
        )
    );
}

#[test]
fn test_strict_only_parens_becomes_literal() {
    // `(` → empty parens removed, but `(` alone is not empty pair.
    // Actually `(` has no matching `)`, and after empty-paren removal it stays.
    // All tokens are non-term → re-interpret as literal
    let f = parse("(").unwrap();
    assert_eq!(f, SearchFilter::Term(FieldTarget::All, "(".into()));
}

#[test]
fn test_strict_empty_parens_is_err() {
    // `()` → empty parens removed → empty → error
    assert!(parse("()").is_err());
    assert!(parse("( )").is_err());
}

#[test]
fn test_strict_tag_with_empty_parens() {
    let f = parse("(#dev) ()").unwrap();
    assert_eq!(f, SearchFilter::Term(FieldTarget::Tag, "dev".into()));
}

#[test]
fn test_strict_trailing_backslash_literal() {
    let f = parse("\\").unwrap();
    assert_eq!(f, SearchFilter::Term(FieldTarget::All, "\\".into()));
}

// === Tolerant parse — preserves old behavior ===

#[test]
fn test_tolerant_empty_returns_none() {
    assert_eq!(parse_tolerant("").unwrap(), None);
    assert_eq!(parse_tolerant("   ").unwrap(), None);
}

#[test]
fn test_tolerant_empty_parens_returns_none() {
    assert_eq!(parse_tolerant("()").unwrap(), None);
    assert_eq!(parse_tolerant("( )").unwrap(), None);
}

#[test]
fn test_tolerant_tag_with_empty_parens() {
    let f = parse_tolerant("(#dev) ( )").unwrap().unwrap();
    assert_eq!(f, SearchFilter::Term(FieldTarget::Tag, "dev".into()));
}

#[test]
fn test_tolerant_trailing_and() {
    let f = parse_tolerant("#dev and").unwrap().unwrap();
    assert_eq!(f, SearchFilter::Term(FieldTarget::Tag, "dev".into()));
}

#[test]
fn test_tolerant_leading_and() {
    let f = parse_tolerant("and #dev").unwrap().unwrap();
    assert_eq!(f, SearchFilter::Term(FieldTarget::Tag, "dev".into()));
}

#[test]
fn test_tolerant_only_and() {
    assert_eq!(parse_tolerant("and").unwrap(), None);
}

#[test]
fn test_tolerant_only_not() {
    assert_eq!(parse_tolerant("not").unwrap(), None);
}

#[test]
fn test_tolerant_unmatched_lparen() {
    assert_eq!(parse_tolerant("(").unwrap(), None);
}

#[test]
fn test_tolerant_unmatched_rparen() {
    assert_eq!(parse_tolerant(")").unwrap(), None);
}

#[test]
fn test_tolerant_unterminated_quote() {
    let f = parse_tolerant("\"hello").unwrap().unwrap();
    assert_eq!(f, SearchFilter::Term(FieldTarget::All, "hello".into()));
}

#[test]
fn test_tolerant_bare_hash() {
    // Bare hash now emits literal "#" word, so tolerant parse gets a term
    let f = parse_tolerant("#").unwrap().unwrap();
    assert_eq!(f, SearchFilter::Term(FieldTarget::All, "#".into()));
}

#[test]
fn test_tolerant_collapsed_adjacent_operators() {
    let f = parse_tolerant("#dev and and #foo").unwrap().unwrap();
    assert_eq!(
        f,
        SearchFilter::And(
            Box::new(SearchFilter::Term(FieldTarget::Tag, "dev".into())),
            Box::new(SearchFilter::Term(FieldTarget::Tag, "foo".into())),
        )
    );
}

#[test]
fn test_tolerant_workspace_merge_empty_keyword() {
    let f = parse_tolerant("(#dev) ()").unwrap().unwrap();
    assert_eq!(f, SearchFilter::Term(FieldTarget::Tag, "dev".into()));
}

// === Regression: all original functional tests (using strict `matches`) ===

#[test]
fn test_parse_simple_word() {
    let f = parse("video").unwrap();
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
fn test_id_prefix_exact() {
    let mut bm = make_bookmark("", "", "", &[]);
    bm.id = 42;
    assert!(matches("=42", &bm).unwrap());
    assert!(!matches("=4", &bm).unwrap());
    assert!(!matches("=abc", &bm).unwrap());
}

#[test]
fn test_required_id_constraint_exact() {
    let f = parse("=42 and rust").unwrap();
    assert_eq!(required_id_constraint(&f), RequiredId::Exact(42));
}

#[test]
fn test_required_id_constraint_unsatisfiable() {
    let f = parse("=42 and =43").unwrap();
    assert_eq!(required_id_constraint(&f), RequiredId::Unsatisfiable);
}

#[test]
fn test_required_id_constraint_or_is_not_strict() {
    let f = parse("=42 or rust").unwrap();
    assert_eq!(required_id_constraint(&f), RequiredId::None);
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

#[test]
fn test_unicode_tag() {
    assert!(matches("#café", &make_bookmark("", "", "", &["café"])).unwrap());
    assert!(matches("#カフェ", &make_bookmark("", "", "", &["カフェ"])).unwrap());
}

#[test]
fn test_unicode_term() {
    assert!(matches("résumé", &make_bookmark("My résumé", "", "", &[])).unwrap());
}

#[test]
fn test_quoted_empty_string() {
    let _ = parse("\"\"");
    let _ = parse(".\"\"");
}

#[test]
fn test_very_long_input() {
    let input = "a ".repeat(5000);
    assert!(parse_tolerant(&input).is_ok());
}

#[test]
fn test_normalize_balance_parens_indirect() {
    // "and () or" — all tokens are operators/empty parens, should normalize to nothing
    let result = parse_tolerant("and () or").unwrap();
    assert!(result.is_none(), "expected None after stripping operators and empty parens");

    // Unmatched parens should be balanced away, leaving the tag filter
    let result = parse_tolerant("(((#dev").unwrap();
    assert!(result.is_some(), "expected Some(tag filter) after balancing parens");
    let filter = result.unwrap();
    let bm_match = make_bookmark("", "", "", &["dev"]);
    let bm_miss = make_bookmark("", "", "", &["other"]);
    assert!(eval(&filter, &bm_match));
    assert!(!eval(&filter, &bm_miss));
}
