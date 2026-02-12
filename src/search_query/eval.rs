use super::parser::{FieldTarget, SearchFilter};
use crate::bookmarks::Bookmark;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RequiredId {
    None,
    Exact(u64),
    Unsatisfiable,
}

pub fn eval(filter: &SearchFilter, bookmark: &Bookmark) -> bool {
    match filter {
        SearchFilter::Term(field, term) => eval_term(field, term, bookmark),
        SearchFilter::And(a, b) => eval(a, bookmark) && eval(b, bookmark),
        SearchFilter::Or(a, b) => eval(a, bookmark) || eval(b, bookmark),
        SearchFilter::Not(inner) => !eval(inner, bookmark),
    }
}

pub fn required_id_constraint(filter: &SearchFilter) -> RequiredId {
    match filter {
        SearchFilter::Term(FieldTarget::Id, term) => match term.parse::<u64>() {
            Ok(id) => RequiredId::Exact(id),
            Err(_) => RequiredId::Unsatisfiable,
        },
        SearchFilter::Term(_, _) => RequiredId::None,
        SearchFilter::And(a, b) => and_required_id(
            required_id_constraint(a),
            required_id_constraint(b),
        ),
        SearchFilter::Or(a, b) => or_required_id(
            required_id_constraint(a),
            required_id_constraint(b),
        ),
        SearchFilter::Not(_) => RequiredId::None,
    }
}

fn and_required_id(left: RequiredId, right: RequiredId) -> RequiredId {
    match (left, right) {
        (RequiredId::Unsatisfiable, _) | (_, RequiredId::Unsatisfiable) => RequiredId::Unsatisfiable,
        (RequiredId::Exact(a), RequiredId::Exact(b)) => {
            if a == b {
                RequiredId::Exact(a)
            } else {
                RequiredId::Unsatisfiable
            }
        }
        (RequiredId::Exact(id), RequiredId::None) | (RequiredId::None, RequiredId::Exact(id)) => {
            RequiredId::Exact(id)
        }
        (RequiredId::None, RequiredId::None) => RequiredId::None,
    }
}

fn or_required_id(left: RequiredId, right: RequiredId) -> RequiredId {
    match (left, right) {
        (RequiredId::Unsatisfiable, r) | (r, RequiredId::Unsatisfiable) => r,
        (RequiredId::Exact(a), RequiredId::Exact(b)) if a == b => RequiredId::Exact(a),
        _ => RequiredId::None,
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
        FieldTarget::Id => term.parse::<u64>().is_ok_and(|id| bm.id == id),
        FieldTarget::All => {
            // Substring across title, description, url; for tags use substring contains
            bm.title.to_lowercase().contains(&term_lower)
                || bm.description.to_lowercase().contains(&term_lower)
                || bm.url.to_lowercase().contains(&term_lower)
                || bm.tags.iter().any(|t| t.to_lowercase().contains(&term_lower))
        }
    }
}
