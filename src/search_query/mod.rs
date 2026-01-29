mod eval;
mod lexer;
mod normalize;
mod parser;

use crate::bookmarks::Bookmark;

pub use eval::eval;
pub use parser::SearchFilter;

/// Parse a keyword query string into a SearchFilter AST.
/// Returns `None` for empty/whitespace-only/operator-only input (match all).
pub fn parse(input: &str) -> anyhow::Result<Option<SearchFilter>> {
    let tokens = lexer::tokenize(input);
    let tokens = normalize::normalize(tokens);
    parser::parse(tokens)
}

/// Convenience: parse + evaluate in one call.
/// Returns true if query is empty (match all).
pub fn matches(query: &str, bookmark: &Bookmark) -> anyhow::Result<bool> {
    match parse(query)? {
        Some(filter) => Ok(eval(&filter, bookmark)),
        None => Ok(true),
    }
}

#[cfg(test)]
mod tests;
