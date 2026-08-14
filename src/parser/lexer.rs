use std::iter::Peekable;
use std::str::{Chars, FromStr};
use strum_macros::EnumString;
use crate::error::{GridScriptError as Error, Result};

#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    Keyword(Keyword),
    Identifier(String),
    UpperName(String),
    IntLiteral(i32),
    FloatLiteral(f32),
    StringLiteral(Vec<u8>),
    Newline, Comma, Colon, LParen, RParen,
    At, Equals, Bang, BangEquals
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, EnumString)]
#[strum(serialize_all = "UPPERCASE")]
pub enum Keyword {
    Start, Go, North, South, East, West, Random,
    Relative, To, This, Direction,
    Checkpoint, Goto, Switch,
    Store, Input, With, Prompt, Load, File,
    Next, Previous, Value, Row,
    Print, Newline, Image,
    Push, Remove, Position, Top, Any, Peek, Home,
    Move, Last, Node, By,
    Increment, Decrement, Giving, Multiply, Divide,
    Shuffle, Call, Arguments, Split, Over,
    Throw, Warn, Return,
    The, Variable, Named,
    Int, Float, #[strum(serialize = "STRING")] Str, Bool, True, False,
}

fn scan_number(chars: &mut Peekable<Chars>) -> Result<Token> {
    let mut buffer = String::new();
    let mut seen_dot = false;

    if let Some(&c) = chars.peek()
        && (c == '-' || c == '+') {
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

    if seen_dot { // float
        buffer.parse::<f32>()
            .map(Token::FloatLiteral)
            .map_err(|_| Error::syntax(format!(
                "invalid float literal '{buffer}'"
            )))
    } else { // int
        buffer.parse::<i32>()
            .map(Token::IntLiteral)
            .map_err(|_| Error::syntax(format!(
                "invalid int literal '{buffer}'"
            )))
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

    let starts_with_digit = buffer.chars().next()
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
            None => return Err(Error::syntax(String::from(
                "unterminated string literal"
            ))),
        }
    }

    Ok(Token::StringLiteral(buffer.into_bytes()))
}

fn scan_bang(chars: &mut Peekable<Chars>) -> Option<Token> {
    chars.next();
    match chars.peek() {
        Some('!') => {
            while let Some(&c) = chars.peek() {
                if c == '\n' { break; }
                chars.next();
            }
            None
        }
        Some('=') => { chars.next(); Some(Token::BangEquals) }
        _ => Some(Token::Bang),
    }
}

pub fn tokenize(source: &str) -> Result<Vec<Token>> {
    let mut chars = source.chars().peekable();
    let mut tokens: Vec<Token> = Vec::new();

    while let Some(&c) = chars.peek() {
        match c {
            ' ' | '\t' | '\r' => { chars.next(); }
            '\n' => { tokens.push(Token::Newline); chars.next(); }
            ','  => { tokens.push(Token::Comma); chars.next(); }
            ':'  => { tokens.push(Token::Colon); chars.next(); }
            '('  => { tokens.push(Token::LParen); chars.next(); }
            ')'  => { tokens.push(Token::RParen); chars.next(); }
            '@'  => { tokens.push(Token::At); chars.next(); }
            '='  => { tokens.push(Token::Equals); chars.next(); }
            '\'' => { tokens.push(scan_string(&mut chars)?); }
            '!' => { if let Some(t) = scan_bang(&mut chars) { tokens.push(t); } }
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
mod tests {
    use super::*;

    #[test]
    fn keyword_parses_from_uppercase() {
        assert_eq!(Keyword::from_str("GO"), Ok(Keyword::Go));
        assert_eq!(Keyword::from_str("CHECKPOINT"), Ok(Keyword::Checkpoint));
        assert!(Keyword::from_str("go").is_err());
        assert!(Keyword::from_str("banana").is_err());
    }

    #[test]
    fn tokenizes_hello_world() {
        let tokens = tokenize("(3,1):PRINT 'Hello World'").unwrap();
        assert_eq!(tokens, vec![
            Token::LParen,
            Token::IntLiteral(3),
            Token::Comma,
            Token::IntLiteral(1),
            Token::RParen,
            Token::Colon,
            Token::Keyword(Keyword::Print),
            Token::StringLiteral(b"Hello World".to_vec()),
        ]);
    }

    #[test]
    fn tokenizes_metadata_line() {
        let tokens = tokenize("@width 10").unwrap();
        assert!(tokens.contains(&Token::IntLiteral(10)));
    }

    #[test]
    fn tokenizes_signed_and_float_numbers() {
        assert_eq!(tokenize("-5").unwrap(), vec![Token::IntLiteral(-5)]);
        assert_eq!(tokenize("3.5").unwrap(), vec![Token::FloatLiteral(3.5)]);
        assert_eq!(tokenize("-2.1").unwrap(), vec![Token::FloatLiteral(-2.1)]);
    }

    #[test]
    fn tokenizes_identifier_vs_keyword() {
        let tokens = tokenize("STORE x").unwrap();
        assert_eq!(tokens, vec![
            Token::Keyword(Keyword::Store),
            Token::Identifier("x".to_string()),
        ]);
    }

    #[test]
    fn rejects_unterminated_string() {
        assert!(tokenize("'unterminated").is_err());
    }

    #[test]
    fn rejects_mixed_case_word() {
        assert!(tokenize("Go").is_err());
    }

    #[test]
    fn scan_word_rejects_leading_digit_directly() {
        let mut chars = "5x".chars().peekable();
        assert!(scan_word(&mut chars).is_err());
    }

    #[test]
    fn digit_then_letter_is_two_tokens() {
        let tokens = tokenize("5x").unwrap();
        assert_eq!(tokens, vec![
            Token::IntLiteral(5),
            Token::Identifier("x".to_string()),
        ]);
    }

    #[test]
    fn rejects_unexpected_character() {
        assert!(tokenize("$").is_err());
    }

    #[test]
    fn newline_produces_token() {
        let tokens = tokenize("GO\nHOME").unwrap();
        assert_eq!(tokens, vec![
            Token::Keyword(Keyword::Go),
            Token::Newline,
            Token::Keyword(Keyword::Home),
        ]);
    }

    #[test]
    fn strips_comments() {
        let tokens = tokenize("HOME !! ignored\nHOME").unwrap();
        assert_eq!(tokens, vec![
            Token::Keyword(Keyword::Home),
            Token::Newline,
            Token::Keyword(Keyword::Home),
        ]);
    }

    #[test]
    fn lexer_switch_operators() {
        assert_eq!(tokenize("!5").unwrap(), vec![Token::Bang, Token::IntLiteral(5)]);
        assert_eq!(tokenize("=5").unwrap(), vec![Token::Equals, Token::IntLiteral(5)]);
        assert_eq!(tokenize("!=5").unwrap(), vec![Token::BangEquals, Token::IntLiteral(5)]);
    }

    #[test]
    fn handles_windows_line_endings() {
        let tokens = tokenize("GO\r\nHOME").unwrap();
        assert_eq!(tokens, vec![
            Token::Keyword(Keyword::Go),
            Token::Newline,
            Token::Keyword(Keyword::Home),
        ]);
    }
}
