mod eval;
mod lexer;
mod normalize;
mod parser;

use crate::bookmarks::Bookmark;

pub use eval::eval;
pub use parser::SearchFilter;

use lexer::Token;

/// Strict parse: returns a SearchFilter or an error.
///
/// Behavior:
/// - Empty/whitespace → error (no query)
/// - Unmatched quotes, bare prefixes → literal words (not errors)
/// - Empty parens `()` → silently removed
/// - All-operator input (`and`, `or not`) → re-interpreted as literal words
/// - Dangling operators with real terms (`#dev and`) → error
/// - Unmatched parens with terms (`(foo`) → error
pub fn parse(input: &str) -> anyhow::Result<SearchFilter> {
    let tokens = lexer::tokenize(input);

    // Remove empty paren groups (only normalization applied in strict mode)
    let tokens = normalize::remove_empty_parens(tokens);

    if tokens.is_empty() {
        anyhow::bail!("empty search query");
    }

    // If all tokens are operators/parens (no terms) → re-interpret as literal Words
    let has_term = tokens.iter().any(|t| {
        matches!(
            t,
            Token::Word(_)
                | Token::QuotedString(_)
                | Token::PrefixedWord(_, _)
                | Token::PrefixedQuoted(_, _)
        )
    });

    let tokens = if !has_term {
        tokens
            .into_iter()
            .map(|t| match t {
                Token::And => Token::Word("and".to_string()),
                Token::Or => Token::Word("or".to_string()),
                Token::Not => Token::Word("not".to_string()),
                Token::LParen => Token::Word("(".to_string()),
                Token::RParen => Token::Word(")".to_string()),
                other => other,
            })
            .collect()
    } else {
        tokens
    };

    parser::parse_strict(tokens)
}

/// Tolerant parse: normalizes away all malformed constructs.
/// Returns `Ok(None)` for empty/whitespace-only/operator-only input (match all).
/// Used by workspace keyword validation/merge where tolerance is required.
pub fn parse_tolerant(input: &str) -> anyhow::Result<Option<SearchFilter>> {
    let tokens = lexer::tokenize(input);
    let tokens = normalize::normalize(tokens);
    parser::parse(tokens)
}

/// Convenience: strict parse + evaluate in one call.
/// Returns error if query is invalid.
pub fn matches(query: &str, bookmark: &Bookmark) -> anyhow::Result<bool> {
    let filter = parse(query)?;
    Ok(eval(&filter, bookmark))
}

#[cfg(test)]
mod tests;
