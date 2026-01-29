use crate::bookmarks::Bookmark;
use super::{parse, eval, matches};

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

// --- Lexer / Parser round-trip ---

#[test]
fn test_parse_simple_word() {
    let f = parse("video").unwrap();
    let bm = make_bookmark("My Video", "", "", &[]);
    assert!(eval(&f, &bm));
}

#[test]
fn test_parse_empty_fails() {
    assert!(parse("").is_err());
    assert!(parse("   ").is_err());
}

#[test]
fn test_unterminated_quote() {
    assert!(parse("\"hello").is_err());
}

#[test]
fn test_unmatched_paren() {
    assert!(parse("(video").is_err());
    assert!(parse("video)").is_err());
}

// --- Field prefixes ---

#[test]
fn test_tag_prefix_exact() {
    let bm = make_bookmark("", "", "", &["video"]);
    assert!(matches("#video", &bm).unwrap());
    assert!(!matches("#vid", &bm).unwrap()); // not substring, must be exact
}

#[test]
fn test_tag_prefix_hierarchical() {
    let bm = make_bookmark("", "", "", &["programming/rust"]);
    assert!(matches("#programming", &bm).unwrap());
    assert!(!matches("#rust", &bm).unwrap()); // rust is child, not parent
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
    assert!(matches("rust", &bm).unwrap());       // title
    assert!(matches("programming", &bm).unwrap()); // description
    assert!(matches("rust-lang", &bm).unwrap());   // url
    assert!(matches("dev", &bm).unwrap());          // tag
}

// --- Quoted phrases ---

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

// --- Boolean operators ---

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
    // "not #archived and rust" = (not #archived) and rust
    let bm = make_bookmark("Rust Guide", "", "", &["programming"]);
    assert!(matches("not #archived and rust", &bm).unwrap());
}

#[test]
fn test_precedence_and_over_or() {
    // "#video and .youtube or #audio" = (#video and .youtube) or #audio
    let bm = make_bookmark("", "", "", &["audio"]);
    assert!(matches("#video and .youtube or #audio", &bm).unwrap());
}

// --- Parentheses ---

#[test]
fn test_parentheses_grouping() {
    let bm = make_bookmark("Spotify Podcast", "", "", &["audio"]);
    // Without parens: #video and (.spotify or #audio) would differ
    assert!(matches("(#video and .spotify) or #audio", &bm).unwrap());
    assert!(!matches("#video and (.spotify or #audio)", &bm).unwrap());
}

// --- Backslash escaping ---

#[test]
fn test_escape_hash() {
    let bm = make_bookmark("#hashtag title", "", "", &[]);
    assert!(matches("\\#hashtag", &bm).unwrap()); // literal #hashtag in all fields
}

#[test]
fn test_escape_dot() {
    let bm = make_bookmark(".dotfile", "", "", &[]);
    assert!(matches("\\.dotfile", &bm).unwrap());
}

// --- Reserved words as literals ---

#[test]
fn test_quoted_reserved_words() {
    let bm = make_bookmark("and or not", "", "", &[]);
    assert!(matches("\"and\"", &bm).unwrap());
    assert!(matches("\"or\"", &bm).unwrap());
    assert!(matches("\"not\"", &bm).unwrap());
}

// --- Case insensitivity ---

#[test]
fn test_case_insensitive() {
    let bm = make_bookmark("RUST Guide", "Learn PYTHON", "https://GITHUB.COM", &["Programming"]);
    assert!(matches("rust", &bm).unwrap());
    assert!(matches("python", &bm).unwrap());
    assert!(matches(":github.com", &bm).unwrap());
    assert!(matches("#programming", &bm).unwrap());
}

// --- Complex queries from RFC examples ---

#[test]
fn test_complex_rfc_example() {
    let bm = make_bookmark("YouTube car video Tutorial", "some desc", "https://youtube.com", &["video"]);
    // Title contains "car video", so `not ."car video"` is false → left side false.
    // #youtube is not a tag → right side also false. Overall: false.
    assert!(!matches("(#video and .youtube and not .\"car video\") or (#youtube and not .\"car video\")", &bm).unwrap());

    // Bookmark where title does NOT contain "car video"
    let bm2 = make_bookmark("YouTube Tutorial", "car video desc", "https://youtube.com", &["video"]);
    // Left: #video ✓, .youtube ✓, not ."car video" ✓ (title lacks it) → true
    assert!(matches("(#video and .youtube and not .\"car video\") or (#youtube and not .\"car video\")", &bm2).unwrap());
}

#[test]
fn test_not_tag_exclude() {
    let bm = make_bookmark("Old Stuff", "", "", &["archived"]);
    assert!(!matches("not #archived", &bm).unwrap());
    let bm2 = make_bookmark("New Stuff", "", "", &["active"]);
    assert!(matches("not #archived", &bm2).unwrap());
}

// --- Tag exact vs all-field substring ---

#[test]
fn test_tag_prefix_vs_all_field() {
    let bm = make_bookmark("", "tagged video content", "", &[]);
    // #video requires tag match, should fail since no tags
    assert!(!matches("#video", &bm).unwrap());
    // bare "video" matches description
    assert!(matches("video", &bm).unwrap());
}

// --- Nested NOT ---

#[test]
fn test_double_not() {
    let bm = make_bookmark("Rust", "", "", &[]);
    assert!(matches("not not rust", &bm).unwrap());
}
