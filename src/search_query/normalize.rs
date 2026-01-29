use super::lexer::Token;

/// Normalize a token stream to remove degenerate constructs that would
/// cause the parser to bail. Never fails.
pub fn normalize(tokens: Vec<Token>) -> Vec<Token> {
    let tokens = remove_empty_parens(tokens);
    let tokens = balance_parens(tokens);
    let tokens = strip_boundary_operators(tokens);
    let tokens = collapse_adjacent_operators(tokens);
    let tokens = strip_dangling_not(tokens);
    tokens
}

/// Remove `LParen RParen` pairs (empty groups).
pub fn remove_empty_parens(mut tokens: Vec<Token>) -> Vec<Token> {
    loop {
        let mut changed = false;
        let mut out = Vec::with_capacity(tokens.len());
        let mut i = 0;
        while i < tokens.len() {
            if i + 1 < tokens.len()
                && tokens[i] == Token::LParen
                && tokens[i + 1] == Token::RParen
            {
                changed = true;
                i += 2;
            } else {
                out.push(tokens[i].clone());
                i += 1;
            }
        }
        tokens = out;
        if !changed {
            break;
        }
    }
    tokens
}

/// Remove unmatched parens.
fn balance_parens(tokens: Vec<Token>) -> Vec<Token> {
    // Forward pass: mark unmatched RParen
    let mut depth: i32 = 0;
    let mut keep = vec![true; tokens.len()];
    for (i, tok) in tokens.iter().enumerate() {
        match tok {
            Token::LParen => depth += 1,
            Token::RParen => {
                if depth > 0 {
                    depth -= 1;
                } else {
                    keep[i] = false;
                }
            }
            _ => {}
        }
    }
    // Backward pass: mark unmatched LParen
    depth = 0;
    for i in (0..tokens.len()).rev() {
        if !keep[i] {
            continue;
        }
        match &tokens[i] {
            Token::RParen => depth += 1,
            Token::LParen => {
                if depth > 0 {
                    depth -= 1;
                } else {
                    keep[i] = false;
                }
            }
            _ => {}
        }
    }
    tokens
        .into_iter()
        .enumerate()
        .filter(|(i, _)| keep[*i])
        .map(|(_, t)| t)
        .collect()
}

/// Strip leading/trailing binary operators (And/Or) and trailing Not.
/// Leading Not is valid (unary prefix), so we keep it.
fn strip_boundary_operators(tokens: Vec<Token>) -> Vec<Token> {
    if tokens.is_empty() {
        return tokens;
    }
    let is_leading_junk = |t: &Token| matches!(t, Token::And | Token::Or);
    let is_trailing_junk = |t: &Token| matches!(t, Token::And | Token::Or | Token::Not);

    let start = tokens.iter().position(|t| !is_leading_junk(t));
    let end = tokens.iter().rposition(|t| !is_trailing_junk(t));
    match (start, end) {
        (Some(s), Some(e)) if s <= e => tokens[s..=e].to_vec(),
        _ => vec![],
    }
}

/// Collapse adjacent operators: `And And` → `And`, `Or Or` → `Or`, etc.
/// Also remove operator before RParen and after LParen.
fn collapse_adjacent_operators(tokens: Vec<Token>) -> Vec<Token> {
    if tokens.is_empty() {
        return tokens;
    }
    let is_operator = |t: &Token| matches!(t, Token::And | Token::Or | Token::Not);
    let mut out: Vec<Token> = Vec::with_capacity(tokens.len());
    for tok in tokens {
        if is_operator(&tok) {
            // Skip operator right after LParen
            if matches!(out.last(), Some(Token::LParen)) && !matches!(tok, Token::Not) {
                continue;
            }
            // Collapse adjacent binary operators (keep Not since `not not` is valid)
            if matches!(tok, Token::And | Token::Or) {
                if matches!(out.last(), Some(Token::And) | Some(Token::Or)) {
                    out.pop();
                }
            }
        }
        if matches!(tok, Token::RParen) {
            // Remove trailing operator before RParen
            while matches!(out.last(), Some(Token::And) | Some(Token::Or) | Some(Token::Not)) {
                out.pop();
            }
        }
        out.push(tok);
    }
    out
}

/// Remove `Not` at end-of-input or before `)`.
fn strip_dangling_not(tokens: Vec<Token>) -> Vec<Token> {
    if tokens.is_empty() {
        return tokens;
    }
    // Already handled by collapse_adjacent_operators + strip_boundary_operators,
    // but run boundary strip again for safety after collapse.
    strip_boundary_operators(tokens)
}
