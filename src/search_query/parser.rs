use anyhow::{bail, Result};

use super::lexer::{Prefix, Token};

#[derive(Debug, Clone, PartialEq)]
pub enum FieldTarget {
    All,
    Tag,
    Title,
    Description,
    Url,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SearchFilter {
    Term(FieldTarget, String),
    And(Box<SearchFilter>, Box<SearchFilter>),
    Or(Box<SearchFilter>, Box<SearchFilter>),
    Not(Box<SearchFilter>),
}

struct Parser {
    tokens: Vec<Token>,
    pos: usize,
}

impl Parser {
    fn new(tokens: Vec<Token>) -> Self {
        Self { tokens, pos: 0 }
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn advance(&mut self) -> Option<Token> {
        let tok = self.tokens.get(self.pos).cloned();
        if tok.is_some() {
            self.pos += 1;
        }
        tok
    }

    fn expect_rparen(&mut self) -> Result<()> {
        match self.advance() {
            Some(Token::RParen) => Ok(()),
            Some(tok) => bail!(
                "expected ')' at position {}, got {}",
                self.pos - 1,
                tok
            ),
            None => bail!("expected closing parenthesis, got end of input"),
        }
    }

    /// or_expr = and_expr ("or" and_expr)*
    fn parse_or(&mut self) -> Result<SearchFilter> {
        let mut left = self.parse_and()?;
        while matches!(self.peek(), Some(Token::Or)) {
            self.advance();
            let right = self.parse_and()?;
            left = SearchFilter::Or(Box::new(left), Box::new(right));
        }
        Ok(left)
    }

    /// and_expr = not_expr (("and")? not_expr)*
    /// Implicit AND: two adjacent terms without "or" between them
    fn parse_and(&mut self) -> Result<SearchFilter> {
        let mut left = self.parse_not()?;
        loop {
            // Explicit "and"
            if matches!(self.peek(), Some(Token::And)) {
                self.advance();
                let right = self.parse_not()?;
                left = SearchFilter::And(Box::new(left), Box::new(right));
                continue;
            }
            // Implicit AND: next token is a term/not/lparen (not or/rparen/end)
            match self.peek() {
                Some(Token::Or) | Some(Token::RParen) | None => break,
                Some(_) => {
                    let right = self.parse_not()?;
                    left = SearchFilter::And(Box::new(left), Box::new(right));
                }
            }
        }
        Ok(left)
    }

    /// not_expr = "not" not_expr | primary
    fn parse_not(&mut self) -> Result<SearchFilter> {
        if matches!(self.peek(), Some(Token::Not)) {
            self.advance();
            let inner = self.parse_not()?;
            return Ok(SearchFilter::Not(Box::new(inner)));
        }
        self.parse_primary()
    }

    /// primary = "(" or_expr ")" | term
    fn parse_primary(&mut self) -> Result<SearchFilter> {
        match self.peek() {
            Some(Token::LParen) => {
                self.advance();
                let expr = self.parse_or()?;
                self.expect_rparen()?;
                Ok(expr)
            }
            Some(
                Token::Word(_)
                | Token::QuotedString(_)
                | Token::PrefixedWord(_, _)
                | Token::PrefixedQuoted(_, _),
            ) => {
                let tok = self.advance().unwrap();
                Ok(token_to_term(tok))
            }
            Some(tok) => bail!("unexpected {} at position {}", tok, self.pos),
            None => bail!("unexpected end of input"),
        }
    }
}

fn prefix_to_field(p: Prefix) -> FieldTarget {
    match p {
        Prefix::Tag => FieldTarget::Tag,
        Prefix::Title => FieldTarget::Title,
        Prefix::Description => FieldTarget::Description,
        Prefix::Url => FieldTarget::Url,
    }
}

fn token_to_term(tok: Token) -> SearchFilter {
    match tok {
        Token::Word(w) => SearchFilter::Term(FieldTarget::All, w),
        Token::QuotedString(s) => SearchFilter::Term(FieldTarget::All, s),
        Token::PrefixedWord(p, w) => SearchFilter::Term(prefix_to_field(p), w),
        Token::PrefixedQuoted(p, s) => SearchFilter::Term(prefix_to_field(p), s),
        _ => unreachable!(),
    }
}

/// Parse normalized tokens into a SearchFilter AST.
/// Returns `Ok(None)` for empty token vec (match all).
pub fn parse(tokens: Vec<Token>) -> Result<Option<SearchFilter>> {
    if tokens.is_empty() {
        return Ok(None);
    }
    let mut parser = Parser::new(tokens);
    let result = parser.parse_or()?;
    if parser.pos < parser.tokens.len() {
        bail!(
            "unexpected {} at position {}",
            parser.tokens[parser.pos],
            parser.pos
        );
    }
    Ok(Some(result))
}
