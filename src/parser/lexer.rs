use crate::error::{GridScriptError as Error, Result};
use std::iter::Peekable;
use std::str::{Chars, FromStr};
use strum_macros::EnumString;

#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    Keyword(Keyword),
    Identifier(String),
    UpperName(String),
    IntLiteral(i32),
    FloatLiteral(f32),
    StringLiteral(Vec<u8>),
    Newline,
    Comma,
    Colon,
    LParen,
    RParen,
    At,
    Equals,
    Bang,
    BangEquals,
    GreaterThan,
    LessThan,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumString)]
#[strum(serialize_all = "UPPERCASE")]
pub enum Keyword {
    Start,
    Go,
    North,
    South,
    East,
    West,
    Random,
    Relative,
    To,
    This,
    Direction,
    Checkpoint,
    Goto,
    Switch,
    Store,
    Input,
    With,
    Prompt,
    Load,
    File,
    Next,
    Previous,
    Value,
    Row,
    Print,
    Newline,
    Image,
    Push,
    Remove,
    Position,
    Top,
    Any,
    Peek,
    Home,
    Move,
    Last,
    Node,
    By,
    Increment,
    Decrement,
    Giving,
    Multiply,
    Divide,
    Shuffle,
    Call,
    Arguments,
    Split,
    Over,
    Throw,
    Warn,
    Return,
    The,
    Variable,
    Named,
    Int,
    Float,
    #[strum(serialize = "STRING")]
    Str,
    Bool,
    True,
    False,
}

fn scan_number(chars: &mut Peekable<Chars>) -> Result<Token> {
    let mut buffer = String::new();
    let mut seen_dot = false;

    if let Some(&c) = chars.peek()
        && (c == '-' || c == '+')
    {
        buffer.push(c);
        chars.next();
    }

    while let Some(&c) = chars.peek() {
        if c.is_ascii_digit() {
            buffer.push(c);
            chars.next();
        } else if c == '.' && !seen_dot {
            seen_dot = true;
            buffer.push(c);
            chars.next();
        } else {
            break;
        }
    }

    if seen_dot {
        // float
        buffer
            .parse::<f32>()
            .map(Token::FloatLiteral)
            .map_err(|_| Error::syntax(format!("invalid float literal '{buffer}'")))
    } else {
        // int
        buffer
            .parse::<i32>()
            .map(Token::IntLiteral)
            .map_err(|_| Error::syntax(format!("invalid int literal '{buffer}'")))
    }
}

fn scan_word(chars: &mut Peekable<Chars>) -> Result<Token> {
    let mut buffer = String::new();

    while let Some(&c) = chars.peek() {
        if c.is_ascii_alphabetic() || c.is_ascii_digit() || c == '_' {
            buffer.push(c);
            chars.next();
        } else {
            break;
        }
    }

    if let Ok(kw) = Keyword::from_str(&buffer) {
        return Ok(Token::Keyword(kw));
    }

    let starts_with_digit = buffer
        .chars()
        .next()
        .map(|c| c.is_ascii_digit())
        .unwrap_or(false);

    if starts_with_digit {
        return Err(Error::syntax(format!(
            "identifiers cannot begin with a digit: '{buffer}'"
        )));
    }

    if buffer == buffer.to_lowercase() {
        return Ok(Token::Identifier(buffer));
    }

    if buffer == buffer.to_uppercase() {
        return Ok(Token::UpperName(buffer));
    }

    Err(Error::syntax(format!(
        "identifiers must be all lowercase or all uppercase: '{buffer}'"
    )))
}

fn scan_string(chars: &mut Peekable<Chars>) -> Result<Token> {
    chars.next(); // consume opening '

    let mut buffer = String::new();

    loop {
        match chars.next() {
            Some('\'') => break, // closing quote
            Some(c) => buffer.push(c),
            None => return Err(Error::syntax(String::from("unterminated string literal"))),
        }
    }

    Ok(Token::StringLiteral(buffer.into_bytes()))
}

fn scan_bang(chars: &mut Peekable<Chars>) -> Option<Token> {
    chars.next();
    match chars.peek() {
        Some('!') => {
            while let Some(&c) = chars.peek() {
                if c == '\n' {
                    break;
                }
                chars.next();
            }
            None
        }
        Some('=') => {
            chars.next();
            Some(Token::BangEquals)
        }
        _ => Some(Token::Bang),
    }
}

fn single_char_token(c: char) -> Option<Token> {
    Some(match c {
        '\n' => Token::Newline,
        ',' => Token::Comma,
        ':' => Token::Colon,
        '(' => Token::LParen,
        ')' => Token::RParen,
        '@' => Token::At,
        '=' => Token::Equals,
        '>' => Token::GreaterThan,
        '<' => Token::LessThan,
        _ => return None,
    })
}

pub fn tokenize(source: &str) -> Result<Vec<Token>> {
    let mut chars = source.chars().peekable();
    let mut tokens: Vec<Token> = Vec::new();

    while let Some(&c) = chars.peek() {
        match c {
            ' ' | '\t' | '\r' => {
                chars.next();
            }
            _ if let Some(t) = single_char_token(c) => {
                tokens.push(t);
                chars.next();
            }
            '\'' => {
                tokens.push(scan_string(&mut chars)?);
            }
            '!' => {
                if let Some(t) = scan_bang(&mut chars) {
                    tokens.push(t);
                }
            }
            _ if c.is_ascii_digit() || c == '-' || c == '+' => {
                tokens.push(scan_number(&mut chars)?);
            }
            _ if c.is_ascii_alphabetic() => {
                tokens.push(scan_word(&mut chars)?);
            }
            _ => {
                return Err(Error::syntax(format!("unexpected character '{c}'")));
            }
        }
    }

    Ok(tokens)
}

#[cfg(test)]
#[path = "../unit/lexer.rs"]
mod tests;
