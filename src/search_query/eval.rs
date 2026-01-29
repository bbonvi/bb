use super::parser::{FieldTarget, SearchFilter};
use crate::bookmarks::Bookmark;

pub fn eval(filter: &SearchFilter, bookmark: &Bookmark) -> bool {
    match filter {
        SearchFilter::Term(field, term) => eval_term(field, term, bookmark),
        SearchFilter::And(a, b) => eval(a, bookmark) && eval(b, bookmark),
        SearchFilter::Or(a, b) => eval(a, bookmark) || eval(b, bookmark),
        SearchFilter::Not(inner) => !eval(inner, bookmark),
    }
}

fn eval_term(field: &FieldTarget, term: &str, bm: &Bookmark) -> bool {
    let term_lower = term.to_lowercase();
    match field {
        FieldTarget::Tag => {
            // Exact match or hierarchical: tag == bm_tag || bm_tag starts with "tag/"
            let tag_prefix = format!("{}/", term_lower);
            bm.tags
                .iter()
                .any(|t| {
                    let t_lower = t.to_lowercase();
                    t_lower == term_lower || t_lower.starts_with(&tag_prefix)
                })
        }
        FieldTarget::Title => bm.title.to_lowercase().contains(&term_lower),
        FieldTarget::Description => bm.description.to_lowercase().contains(&term_lower),
        FieldTarget::Url => bm.url.to_lowercase().contains(&term_lower),
        FieldTarget::All => {
            // Substring across title, description, url; for tags use substring contains
            bm.title.to_lowercase().contains(&term_lower)
                || bm.description.to_lowercase().contains(&term_lower)
                || bm.url.to_lowercase().contains(&term_lower)
                || bm.tags.iter().any(|t| t.to_lowercase().contains(&term_lower))
        }
    }
}
