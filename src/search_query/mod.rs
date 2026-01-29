mod eval;
mod lexer;
mod parser;

use crate::bookmarks::Bookmark;

pub use eval::eval;
pub use parser::SearchFilter;

/// Parse a keyword query string into a SearchFilter AST.
pub fn parse(input: &str) -> anyhow::Result<SearchFilter> {
    let tokens = lexer::tokenize(input)?;
    parser::parse(tokens)
}

/// Convenience: parse + evaluate in one call.
pub fn matches(query: &str, bookmark: &Bookmark) -> anyhow::Result<bool> {
    let filter = parse(query)?;
    Ok(eval(&filter, bookmark))
}

#[cfg(test)]
mod tests;
