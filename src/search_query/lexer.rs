use anyhow::{bail, Result};

#[derive(Debug, Clone, PartialEq)]
pub enum Prefix {
    Tag,         // #
    Title,       // .
    Description, // >
    Url,         // :
}

#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    Word(String),
    QuotedString(String),
    PrefixedWord(Prefix, String),
    PrefixedQuoted(Prefix, String),
    And,
    Or,
    Not,
    LParen,
    RParen,
}

pub fn tokenize(input: &str) -> Result<Vec<Token>> {
    let mut tokens = Vec::new();
    let chars: Vec<char> = input.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        match chars[i] {
            ' ' | '\t' | '\n' | '\r' => {
                i += 1;
            }
            '(' => {
                tokens.push(Token::LParen);
                i += 1;
            }
            ')' => {
                tokens.push(Token::RParen);
                i += 1;
            }
            '"' => {
                let s = read_quoted(&chars, &mut i)?;
                tokens.push(Token::QuotedString(s));
            }
            '#' | '.' | '>' | ':' => {
                let prefix = match chars[i] {
                    '#' => Prefix::Tag,
                    '.' => Prefix::Title,
                    '>' => Prefix::Description,
                    ':' => Prefix::Url,
                    _ => unreachable!(),
                };
                i += 1;
                if i < len && chars[i] == '"' {
                    let s = read_quoted(&chars, &mut i)?;
                    tokens.push(Token::PrefixedQuoted(prefix, s));
                } else {
                    let word = read_word(&chars, &mut i);
                    if word.is_empty() {
                        bail!("expected word after prefix character at position {}", i);
                    }
                    tokens.push(Token::PrefixedWord(prefix, word));
                }
            }
            '\\' => {
                // Backslash escaping
                i += 1;
                if i >= len {
                    bail!("unexpected end of input after backslash");
                }
                let escaped_char = chars[i];
                i += 1;
                // Read rest of word after the escaped char
                let rest = read_word(&chars, &mut i);
                let word = format!("{}{}", escaped_char, rest);
                tokens.push(Token::Word(word));
            }
            _ => {
                let word = read_word(&chars, &mut i);
                match word.as_str() {
                    "and" => tokens.push(Token::And),
                    "or" => tokens.push(Token::Or),
                    "not" => tokens.push(Token::Not),
                    _ => tokens.push(Token::Word(word)),
                }
            }
        }
    }

    Ok(tokens)
}

fn read_quoted(chars: &[char], i: &mut usize) -> Result<String> {
    let start = *i;
    *i += 1; // skip opening quote
    let mut s = String::new();
    while *i < chars.len() {
        if chars[*i] == '\\' && *i + 1 < chars.len() {
            *i += 1;
            s.push(chars[*i]);
            *i += 1;
            continue;
        }
        if chars[*i] == '"' {
            *i += 1; // skip closing quote
            return Ok(s);
        }
        s.push(chars[*i]);
        *i += 1;
    }
    bail!("unterminated quoted string starting at position {}", start);
}

fn read_word(chars: &[char], i: &mut usize) -> String {
    let mut word = String::new();
    while *i < chars.len() {
        match chars[*i] {
            ' ' | '\t' | '\n' | '\r' | '(' | ')' | '"' => break,
            _ => {
                word.push(chars[*i]);
                *i += 1;
            }
        }
    }
    word
}
