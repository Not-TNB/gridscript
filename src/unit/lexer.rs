//! Unit tests for `parser::lexer`

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
    assert_eq!(
        tokens,
        vec![
            Token::LParen,
            Token::IntLiteral(3),
            Token::Comma,
            Token::IntLiteral(1),
            Token::RParen,
            Token::Colon,
            Token::Keyword(Keyword::Print),
            Token::StringLiteral(b"Hello World".to_vec()),
        ]
    );
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
    assert_eq!(
        tokens,
        vec![
            Token::Keyword(Keyword::Store),
            Token::Identifier("x".to_string()),
        ]
    );
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
    assert_eq!(
        tokens,
        vec![Token::IntLiteral(5), Token::Identifier("x".to_string()),]
    );
}

#[test]
fn rejects_unexpected_character() {
    assert!(tokenize("$").is_err());
}

#[test]
fn newline_produces_token() {
    let tokens = tokenize("GO\nHOME").unwrap();
    assert_eq!(
        tokens,
        vec![
            Token::Keyword(Keyword::Go),
            Token::Newline,
            Token::Keyword(Keyword::Home),
        ]
    );
}

#[test]
fn strips_comments() {
    let tokens = tokenize("HOME !! ignored\nHOME").unwrap();
    assert_eq!(
        tokens,
        vec![
            Token::Keyword(Keyword::Home),
            Token::Newline,
            Token::Keyword(Keyword::Home),
        ]
    );
}

#[test]
fn lexer_switch_operators() {
    assert_eq!(
        tokenize("!5").unwrap(),
        vec![Token::Bang, Token::IntLiteral(5)]
    );
    assert_eq!(
        tokenize("=5").unwrap(),
        vec![Token::Equals, Token::IntLiteral(5)]
    );
    assert_eq!(
        tokenize("!=5").unwrap(),
        vec![Token::BangEquals, Token::IntLiteral(5)]
    );
}

#[test]
fn handles_windows_line_endings() {
    let tokens = tokenize("GO\r\nHOME").unwrap();
    assert_eq!(
        tokens,
        vec![
            Token::Keyword(Keyword::Go),
            Token::Newline,
            Token::Keyword(Keyword::Home),
        ]
    );
}
