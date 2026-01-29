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

impl std::fmt::Display for Token {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Token::Word(w) => write!(f, "'{}'", w),
            Token::QuotedString(s) => write!(f, "\"{}\"", s),
            Token::PrefixedWord(_, w) => write!(f, "'{}'", w),
            Token::PrefixedQuoted(_, s) => write!(f, "\"{}\"", s),
            Token::And => write!(f, "'and'"),
            Token::Or => write!(f, "'or'"),
            Token::Not => write!(f, "'not'"),
            Token::LParen => write!(f, "'('"),
            Token::RParen => write!(f, "')'"),
        }
    }
}

/// Tokenize input. Never fails — malformed input is handled tolerantly.
pub fn tokenize(input: &str) -> Vec<Token> {
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
                let s = read_quoted(&chars, &mut i);
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
                    let s = read_quoted(&chars, &mut i);
                    tokens.push(Token::PrefixedQuoted(prefix, s));
                } else {
                    let word = read_word(&chars, &mut i);
                    if word.is_empty() {
                        // Bare prefix with no following word — emit as literal
                        let literal = match prefix {
                            Prefix::Tag => "#",
                            Prefix::Title => ".",
                            Prefix::Description => ">",
                            Prefix::Url => ":",
                        };
                        tokens.push(Token::Word(literal.to_string()));
                    } else {
                        tokens.push(Token::PrefixedWord(prefix, word));
                    }
                }
            }
            '\\' => {
                i += 1;
                if i >= len {
                    // Trailing backslash — emit as literal
                    tokens.push(Token::Word("\\".to_string()));
                    break;
                }
                let escaped_char = chars[i];
                i += 1;
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

    tokens
}

/// Read a quoted string. Tolerant: unterminated quote treats rest-of-input as the string.
fn read_quoted(chars: &[char], i: &mut usize) -> String {
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
            return s;
        }
        s.push(chars[*i]);
        *i += 1;
    }
    // Unterminated — return what we have
    s
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
