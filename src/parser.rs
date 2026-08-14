pub mod ast;
pub mod lexer;

use crate::error::{GridScriptError as Error, Result};
use crate::parser::ast::{
    ArithOp, Command, DebugMode, GoTarget, GotoTarget,
    RawMetadata, StoreSource, SwitchCond, ValueExpr
};
use crate::parser::lexer::{Keyword, Token};
use crate::program::Program;
use crate::types::{DataType, Value};

/* ---------------------------------------------------------------------------------------------
 * PARSER STRUCT
 * --------------------------------------------------------------------------------------------- */

struct Parser<'a> {
    tokens: &'a [Token],
    pos: usize,
}

impl<'a> Parser<'a> {
    fn new(tokens: &'a [Token]) -> Self {
        Parser { tokens, pos: 0 }
    }

    /// The token at the cursor, if any.
    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    /// The token `n` positions ahead of the cursor, if any.
    fn peek_at(&self, n: usize) -> Option<&Token> {
        self.tokens.get(self.pos + n)
    }

    /// Moves the cursor forward one token.
    fn advance(&mut self) {
        self.pos += 1;
    }

    /// True if the cursor is past the last token.
    fn at_end(&self) -> bool {
        self.pos >= self.tokens.len()
    }

    /// The keyword at the cursor, if there is one.
    fn keyword(&self) -> Option<Keyword> {
        match self.peek() {
            Some(Token::Keyword(k)) => Some(*k),
            _ => None,
        }
    }

    /// True if the cursor is at the given keyword.
    fn at_keyword(&self, kw: Keyword) -> bool {
        self.keyword() == Some(kw)
    }

    /// Consumes the given keyword, erroring if it isn't there.
    fn expect_keyword(&mut self, kw: Keyword) -> Result<()> {
        if self.at_keyword(kw) {
            self.advance();
            Ok(())
        } else {
            Err(Error::syntax(format!("expected {kw:?}, found {:?}", self.peek())))
        }
    }

    /// Consumes the given keyword if present; reports whether it was.
    fn eat_keyword(&mut self, kw: Keyword) -> bool {
        if self.at_keyword(kw) {
            self.advance();
            true
        } else { false }
    }
}

/* ---------------------------------------------------------------------------------------------
 * GENERAL HELPERS AND HELPERS FOR METADATA / VALUE_EXPR PARSING
 * --------------------------------------------------------------------------------------------- */

/// The keyword at the front of `tokens`, if there is one.
fn keyword(tokens: &[Token]) -> Option<Keyword> {
    match tokens.first() {
        Some(Token::Keyword(k)) => Some(*k),
        _ => None,
    }
}

/// True if `tokens` starts with the given keyword.
fn at_keyword(tokens: &[Token], kw: Keyword) -> bool {
    keyword(tokens) == Some(kw)
}

/// If `tokens` starts with `kw`, consumes it and parses the following value expression.
/// Consumes nothing otherwise.
fn parse_optional_clause(tokens: &[Token], kw: Keyword) -> Result<(Option<ValueExpr>, &[Token])> {
    if at_keyword(tokens, kw) {
        let (expr, rest) = parse_value_expr(&tokens[1..])?;
        Ok((Some(expr), rest))
    } else {
        Ok((None, tokens))
    }
}

/// Advances past any leading `Newline` tokens.
fn skip_newlines(tokens: &[Token]) -> &[Token] {
    let mut rest = tokens;
    while matches!(rest.first(), Some(Token::Newline)) {
        rest = &rest[1..];
    }
    rest
}

/// Splits a leading title line (`#TITLE.` or `##TITLE.`) off the front of
/// `source`, returning the title text and the remaining source.
fn split_title(source: &str, octothorpes: usize) -> Result<(&str, &str)> {
    let (line, rest) = match source.split_once('\n') {
        Some((line, rest)) => (line, rest),
        None => (source, ""),
    };

    let line = line.trim();

    let prefix = "#".repeat(octothorpes);
    let body = line
        .strip_prefix(&prefix)
        .ok_or_else(|| Error::syntax(format!(
            "expected a title line beginning with '{prefix}', found '{line}'"
        )))?;

    if body.starts_with('#') {
        return Err(Error::syntax(format!(
            "too many leading octothorpes in title line '{line}'"
        )));
    }

    let title = body
        .strip_suffix('.')
        .ok_or_else(|| Error::syntax(format!(
            "title line must end with a period, found '{line}'"
        )))?;

    Ok((title.trim(), rest))
}

/// Reads one integer literal associated with a key, widening it to `i64`.
fn expect_int<'a>(tokens: &'a [Token], key: &str) -> Result<(i64, &'a [Token])> {
    match tokens.first() {
        Some(Token::IntLiteral(n)) => Ok((*n as i64, &tokens[1..])),
        other => Err(Error::syntax(format!(
            "metadata key '{key}' expects an integer value, found {other:?}"
        ))),
    }
}

/// Reads one `true`/`false`/`auto` value associated with `@debug`.
fn expect_debug_mode(tokens: &[Token]) -> Result<(DebugMode, &[Token])> {
    let mode = match tokens.first() {
        Some(Token::Identifier(s)) if s == "true"  => DebugMode::True,
        Some(Token::Identifier(s)) if s == "false" => DebugMode::False,
        Some(Token::Identifier(s)) if s == "auto"  => DebugMode::Auto,
        other => return Err(Error::syntax(format!(
            "metadata key 'debug' expects true, false, or auto, found {other:?}"
        ))),
    };
    Ok((mode, &tokens[1..]))
}

/// Parses a single `@key value` line into `raw`, returning the remaining tokens.
/// PRE: `tokens` starts with `Token::At`
fn parse_metadata_line<'a>(raw: &mut RawMetadata, tokens: &'a [Token]) -> Result<&'a [Token]> {
    let mut rest = &tokens[1..]; // consume @

    // read key
    let key = match rest.first() {
        Some(Token::Identifier(name)) => name.clone(),
        other => return Err(Error::syntax(format!(
            "expected an identifier, found '{:?}'", other
        ))),
    };

    rest = &rest[1..]; // consume key

    macro_rules! int_key {
        ($field:ident) => {{
            let (v, r) = expect_int(rest, &key)?;
            raw.$field = Some(v); r
        }};
    }

    // consume value based on key
    rest = match key.as_str() {
        "width"      => int_key!(width),
        "height"     => int_key!(height),
        "datawidth"  => int_key!(data_width),
        "dataheight" => int_key!(data_height),
        "radius"     => int_key!(radius),
        "steps"      => int_key!(steps),
        "maxdepth"   => int_key!(max_depth),
        "seed" => {
            let (v, r) = expect_int(rest, "seed")?;
            raw.seed = Some(v as u64); r
        },
        "debug" => {
            let (v, r) = expect_debug_mode(rest)?;
            raw.debug = Some(v); r
        }
        _ => return Err(Error::syntax(format!("unknown metadata key '{key}'"))),
    };

    // check for newline
    match rest.first() {
        Some(Token::Newline) => rest = &rest[1..],
        Some(other) => return Err(Error::syntax(format!(
            "expected end of metadata line, found {other:?}"
        ))),
        None => (),
    }

    Ok(rest)
}

/// Parses a single value expression of the form `THE _ NAMED _`
/// PRE: `tokens` starts with `Token::Keyword(Keyword::The)`
fn parse_dynamic_value_expr(tokens: &[Token]) -> Result<(ValueExpr, &[Token])> {
    let mut rest = &tokens[1..];

    let cast = match keyword(rest) {
        Some(Keyword::Variable) => None,
        Some(Keyword::Int)      => Some(DataType::Int),
        Some(Keyword::Float)    => Some(DataType::Float),
        Some(Keyword::Str)      => Some(DataType::Str),
        Some(Keyword::Bool)     => Some(DataType::Bool),
        _ => return Err(Error::syntax(format!(
            "expected VARIABLE or a type after THE, found {:?}", rest.first()
        ))),
    };
    rest = &rest[1..];

    if !at_keyword(rest, Keyword::Named) {
        return Err(Error::syntax(format!("expected NAMED, found {:?}", rest.first())));
    }
    rest = &rest[1..];

    let (name, rest) = parse_value_expr(rest)?;

    Ok((ValueExpr::DynamicVar {name: Box::new(name), cast}, rest))
}

/* ---------------------------------------------------------------------------------------------
 * HELPERS FOR COMMAND PARSING
 * --------------------------------------------------------------------------------------------- */

/// Parses either a `VALUE` or a `ROW`, producing a given command.
fn parse_value_or_row(
    tokens: &[Token], on_value: Command, on_row: Command,
) -> Result<(Command, &[Token])> {
    match keyword(&tokens[1..]) {
        Some(Keyword::Value) => Ok((on_value, &tokens[2..])),
        Some(Keyword::Row) => Ok((on_row, &tokens[2..])),
        _ => Err(Error::syntax(format!(
            "expected VALUE or ROW, found {:?}", tokens.get(1)
        ))),
    }
}

/// Parses `PUSH [value]`.
/// PRE: `tokens` starts with `Token::Keyword(Keyword::Push)`
fn parse_push(tokens: &[Token]) -> Result<(Command, &[Token])> {
    let rest = &tokens[1..];
    match rest.first() {
        None | Some(Token::Newline) => Ok((Command::Push(None), rest)),
        _ => {
            let (value, rest) = parse_value_expr(rest)?;
            Ok((Command::Push(Some(value)), rest))
        }
    }
}

/// Parses `GOTO id|THIS CHECKPOINT`.
/// PRE: `tokens` starts with `Token::Keyword(Keyword::Goto)`
fn parse_goto(tokens: &[Token]) -> Result<(Command, &[Token])> {
    let rest = &tokens[1..];
    if at_keyword(rest, Keyword::This) {
        if at_keyword(&rest[1..], Keyword::Checkpoint) {
            Ok((Command::Goto(GotoTarget::ThisCheckpoint), &rest[2..]))
        } else {
            Err(Error::syntax(format!("expected CHECKPOINT, found {:?}", rest.get(1))))
        }
    } else {
        let (id, rest) = parse_value_expr(rest)?;
        Ok((Command::Goto(GotoTarget::Id(id)), rest))
    }
}

/// Parses `INCREMENT|DECREMENT|MULTIPLY|DIVIDE [var] [BY value] [GIVING var]`.
/// PRE: `tokens` starts with the operation keyword
fn parse_arithmetic(tokens: &[Token], op: ArithOp) -> Result<(Command, &[Token])> {
    let mut rest = &tokens[1..];

    // optional target
    let target = if at_keyword(rest, Keyword::By)
        || at_keyword(rest, Keyword::Giving)
        || matches!(rest.first(), None | Some(&Token::Newline))
    { None } else {
        let (expr, r) = parse_value_expr(rest)?; rest = r;
        Some(expr)
    };

    let (by, rest) = parse_optional_clause(rest, Keyword::By)?;
    let (giving, rest) = parse_optional_clause(rest, Keyword::Giving)?;

    if by.is_none() && matches!(op, ArithOp::Multiply | ArithOp::Divide) {
        return Err(Error::syntax(format!("{op:?} requires a BY clause")));
    }

    Ok((Command::Arithmetic { op, target, by, giving }, rest))
}

/// Parses `GO NORTH|SOUTH|EAST|WEST|RANDOM|[RELATIVE TO] THIS DIRECTION|[RELATIVE TO] value`.
/// PRE: `tokens` starts with `Token::Keyword(Keyword::Go)`
fn parse_go(tokens: &[Token]) -> Result<(Command, &[Token])> {
    let mut rest = &tokens[1..];

    // optional RELATIVE TO
    let relative = if at_keyword(rest, Keyword::Relative) {
        if !at_keyword(&rest[1..], Keyword::To) {
            return Err(Error::syntax(format!(
                "expected TO after RELATIVE, found {:?}", rest.get(1)
            )));
        }
        rest = &rest[2..]; true
    } else { false };

    // target: THIS DIRECTION, N/E/W/S, RANDOM
    let (target, rest) = match keyword(rest) {
        Some(Keyword::North)  => (GoTarget::North, &rest[1..]),
        Some(Keyword::South)  => (GoTarget::South, &rest[1..]),
        Some(Keyword::East)   => (GoTarget::East, &rest[1..]),
        Some(Keyword::West)   => (GoTarget::West, &rest[1..]),
        Some(Keyword::Random) => (GoTarget::Random, &rest[1..]),
        Some(Keyword::This) => {
            if at_keyword(&rest[1..], Keyword::Direction) {
                (GoTarget::ThisDirection, &rest[2..])
            } else {
                return Err(Error::syntax(format!(
                    "expected DIRECTION after THIS, found {:?}", rest.get(1)
                )));
            }
        }
        _ => { // fallback to value
            let (expr, r) = parse_value_expr(rest)?;
            (GoTarget::Value(expr), r)
        }
    };

    if relative && !matches!(target, GoTarget::ThisDirection | GoTarget::Value(_)) {
        return Err(Error::syntax(format!(
            "cannot make {:?} a relative target", target
        )));
    }

    Ok((Command::Go { target, relative }, rest))
}

/// Parses `SWITCH RANDOM|value|!value|=value|!=value`.
/// PRE: `tokens` starts with `Token::Keyword(Keyword::Switch)`
fn parse_switch(tokens: &[Token]) -> Result<(Command, &[Token])> {
    let rest = &tokens[1..];

    if at_keyword(rest, Keyword::Random) {
        return Ok((Command::Switch(SwitchCond::Random), &rest[1..]));
    }

    let (build, rest): (fn(ValueExpr) -> SwitchCond, &[Token]) = match rest.first() {
        Some(Token::Bang) => (SwitchCond::Falsy, &rest[1..]),
        Some(Token::Equals) => (SwitchCond::Equals, &rest[1..]),
        Some(Token::BangEquals) => (SwitchCond::NotEquals, &rest[1..]),
        _ => (SwitchCond::Truthy, rest),
    };

    let (expr, rest) = parse_value_expr(rest)?;
    Ok((Command::Switch(build(expr)), rest))
}

/// Parses `STORE value|RANDOM [TO variable]`.
/// PRE: `tokens` startts with `Token::Keyword(Keyword::Store)`
fn parse_store(tokens: &[Token]) -> Result<(Command, &[Token])> {
    let rest = &tokens[1..];

    let (source, rest) = if at_keyword(rest, Keyword::Random) {
        (StoreSource::Random, &rest[1..])
    } else {
        let (expr, rest) = parse_value_expr(rest)?;
        (StoreSource::Value(expr), rest)
    };

    let (target, rest) = parse_optional_clause(rest, Keyword::To)?;
    Ok((Command::Store { source, target }, rest))
}

/* ---------------------------------------------------------------------------------------------
 * PARSING COMPONENTS
 * --------------------------------------------------------------------------------------------- */

/// Parses the whole `@key value` metadata block.
fn parse_metadata(tokens: &[Token]) -> Result<(RawMetadata, &[Token])> {
    let mut rest = skip_newlines(tokens);
    let mut raw = RawMetadata::default();
    while let Some(Token::At) = rest.first() {
        rest = parse_metadata_line(&mut raw, rest)?;
    }
    Ok((raw, rest))
}

/// Parses one value expression (literal, variable reference or a dynamic `THE _ NAMED _` form)
fn parse_value_expr(tokens: &[Token]) -> Result<(ValueExpr, &[Token])> {
    let expr = match tokens.first() {
        Some(Token::IntLiteral(n)) => ValueExpr::Literal(Value::Int(*n)),
        Some(Token::FloatLiteral(f)) => ValueExpr::Literal(Value::Float(*f)),
        Some(Token::StringLiteral(s)) => ValueExpr::Literal(Value::Str(s.clone())),
        Some(Token::Keyword(Keyword::True)) => ValueExpr::Literal(Value::Bool(true)),
        Some(Token::Keyword(Keyword::False)) => ValueExpr::Literal(Value::Bool(false)),
        Some(Token::Identifier(name)) => ValueExpr::Var(name.clone()),
        Some(Token::Keyword(Keyword::The)) => return parse_dynamic_value_expr(tokens),
        other => return Err(Error::syntax(format!(
            "expected a value, found {other:?}"
        )))
    };
    Ok((expr, &tokens[1..]))
}

/// Parses one command
fn parse_command(tokens: &[Token]) -> Result<(Command, &[Token])> {
    match keyword(tokens) {
        Some(Keyword::Start)   => Ok((Command::Start, &tokens[1..])),
        Some(Keyword::Home)    => Ok((Command::Home, &tokens[1..])),
        Some(Keyword::Shuffle) => Ok((Command::Shuffle, &tokens[1..])),

        Some(Keyword::Next) =>
            parse_value_or_row(tokens, Command::NextValue, Command::NextRow),
        Some(Keyword::Previous) =>
            parse_value_or_row(tokens, Command::PreviousValue, Command::PreviousRow),

        Some(Keyword::Throw) => parse_value_expr(&tokens[1..])
            .map(|(expr, rest)| (Command::Throw(expr), rest)),
        Some(Keyword::Warn) => parse_value_expr(&tokens[1..])
            .map(|(expr, rest)| (Command::Warn(expr), rest)),

        Some(Keyword::Push) => parse_push(tokens),
        Some(Keyword::Goto) => parse_goto(tokens),

        Some(Keyword::Increment) => parse_arithmetic(tokens, ArithOp::Increment),
        Some(Keyword::Decrement) => parse_arithmetic(tokens, ArithOp::Decrement),
        Some(Keyword::Multiply)  => parse_arithmetic(tokens, ArithOp::Multiply),
        Some(Keyword::Divide)    => parse_arithmetic(tokens, ArithOp::Divide),

        Some(Keyword::Go)     => parse_go(tokens),
        Some(Keyword::Switch) => parse_switch(tokens),
        Some(Keyword::Store)  => parse_store(tokens),

        _ => Err(Error::syntax(format!(
            "expected a command, found {:?}", tokens.first()
        )))
    }
}

/* ---------------------------------------------------------------------------------------------
 * MAIN PARSER
 * --------------------------------------------------------------------------------------------- */

/// Parses a full GridScript source file into a `Program`.
pub fn parse(_source: &str) -> Result<Program> {
    todo!("implement parser")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_main_title() {
        let (title, rest) = split_title(
            "#HELLO WORLD.\n\n@width 4", 1
        ).unwrap();
        assert_eq!(title, "HELLO WORLD");
        assert_eq!(rest, "\n@width 4");
    }

    #[test]
    fn rejects_subroutine_title_as_main() {
        assert!(split_title("##ACK.\n", 1).is_err());
    }

    #[test]
    fn requires_trailing_period() {
        assert!(split_title("#NO PERIOD\n", 1).is_err());
    }

    #[test]
    fn parses_metadata_block() {
        let tokens = lexer::tokenize(
            "\n@width 4\n@height 1\n@debug auto\n"
        ).unwrap();
        let (raw, _) = parse_metadata(&tokens).unwrap();
        assert_eq!(raw.width, Some(4));
        assert_eq!(raw.height, Some(1));
        assert_eq!(raw.debug, Some(DebugMode::Auto));
    }

    #[test]
    fn rejects_unknown_metadata_key() {
        let tokens = lexer::tokenize("@bogus 5\n").unwrap();
        assert!(parse_metadata(&tokens).is_err());
    }

    #[test]
    fn parses_literals() {
        let cases: Vec<(&str, Value)> = vec![
            ("42", Value::Int(42)),
            ("-7", Value::Int(-7)),
            ("3.5", Value::Float(3.5)),
            ("'hi'", Value::Str(b"hi".to_vec())),
            ("TRUE", Value::Bool(true)),
            ("FALSE", Value::Bool(false)),
        ];

        for (source, expected) in cases {
            let tokens = lexer::tokenize(source).unwrap();
            let (expr, rest) = parse_value_expr(&tokens).unwrap();
            assert_eq!(expr, ValueExpr::Literal(expected));
            assert!(rest.is_empty());
        }
    }

    #[test]
    fn parses_variable_reference() {
        let tokens = lexer::tokenize("count_2").unwrap();
        let (expr, rest) = parse_value_expr(&tokens).unwrap();
        assert_eq!(expr, ValueExpr::Var("count_2".to_string()));
        assert!(rest.is_empty());
    }

    #[test]
    fn parses_dynamic_variable_untyped() {
        let tokens = lexer::tokenize("THE VARIABLE NAMED 'x'").unwrap();
        let (expr, rest) = parse_value_expr(&tokens).unwrap();
        assert_eq!(expr, ValueExpr::DynamicVar {
            name: Box::new(ValueExpr::Literal(Value::Str(b"x".to_vec()))),
            cast: None,
        });
        assert!(rest.is_empty());
    }

    #[test]
    fn parses_dynamic_variable_typed() {
        let tokens = lexer::tokenize("THE INT NAMED holder").unwrap();
        let (expr, _) = parse_value_expr(&tokens).unwrap();
        assert_eq!(expr, ValueExpr::DynamicVar {
            name: Box::new(ValueExpr::Var("holder".to_string())),
            cast: Some(DataType::Int),
        });
    }

    #[test]
    fn parses_nested_dynamic_variable() {
        let tokens = lexer::tokenize(
            "THE VARIABLE NAMED THE STRING NAMED 'outer'"
        ).unwrap();
        let (expr, _) = parse_value_expr(&tokens).unwrap();
        assert_eq!(expr, ValueExpr::DynamicVar {
            name: Box::new(ValueExpr::DynamicVar {
                name: Box::new(ValueExpr::Literal(Value::Str(b"outer".to_vec()))),
                cast: Some(DataType::Str),
            }),
            cast: None,
        });
    }

    #[test]
    fn leaves_trailing_tokens_unconsumed() {
        let tokens = lexer::tokenize("5 GIVING x").unwrap();
        let (expr, rest) = parse_value_expr(&tokens).unwrap();
        assert_eq!(expr, ValueExpr::Literal(Value::Int(5)));
        assert_eq!(rest.first(), Some(&Token::Keyword(Keyword::Giving)));
    }

    #[test]
    fn rejects_malformed_dynamic_forms() {
        // missing NAMED
        assert!(parse_value_expr(&lexer::tokenize("THE VARIABLE 'x'").unwrap()).is_err());
        // bad slot after THE
        assert!(parse_value_expr(&lexer::tokenize("THE NAMED 'x'").unwrap()).is_err());
        // nothing after THE
        assert!(parse_value_expr(&lexer::tokenize("THE").unwrap()).is_err());
    }

    #[test]
    fn rejects_non_value_token() {
        assert!(parse_value_expr(&lexer::tokenize(":").unwrap()).is_err());
        assert!(parse_value_expr(&[]).is_err());
    }

    #[test]
    fn parses_zero_argument_commands() {
        let cases = vec![
            ("START", Command::Start),
            ("HOME", Command::Home),
            ("SHUFFLE", Command::Shuffle),
            ("NEXT VALUE", Command::NextValue),
            ("NEXT ROW", Command::NextRow),
            ("PREVIOUS VALUE", Command::PreviousValue),
            ("PREVIOUS ROW", Command::PreviousRow),
        ];
        for (source, expected) in cases {
            let tokens = lexer::tokenize(source).unwrap();
            let (cmd, rest) = parse_command(&tokens).unwrap();
            assert_eq!(cmd, expected);
            assert!(rest.is_empty());
        }
    }

    #[test]
    fn parses_throw_and_warn() {
        let tokens = lexer::tokenize("THROW 'boom'").unwrap();
        let (cmd, _) = parse_command(&tokens).unwrap();
        assert_eq!(cmd, Command::Throw(ValueExpr::Literal(Value::Str(b"boom".to_vec()))));

        let tokens = lexer::tokenize("WARN x").unwrap();
        let (cmd, _) = parse_command(&tokens).unwrap();
        assert_eq!(cmd, Command::Warn(ValueExpr::Var("x".to_string())));
    }

    #[test]
    fn parses_push_with_and_without_argument() {
        let tokens = lexer::tokenize("PUSH 5").unwrap();
        let (cmd, _) = parse_command(&tokens).unwrap();
        assert_eq!(cmd, Command::Push(Some(ValueExpr::Literal(Value::Int(5)))));

        let tokens = lexer::tokenize("PUSH").unwrap();
        let (cmd, _) = parse_command(&tokens).unwrap();
        assert_eq!(cmd, Command::Push(None));

        let tokens = lexer::tokenize("PUSH\nHOME").unwrap();
        let (cmd, rest) = parse_command(&tokens).unwrap();
        assert_eq!(cmd, Command::Push(None));
        assert_eq!(rest.first(), Some(&Token::Newline));
    }

    #[test]
    fn rejects_incomplete_next() {
        assert!(parse_command(&lexer::tokenize("NEXT").unwrap()).is_err());
        assert!(parse_command(&lexer::tokenize("NEXT HOME").unwrap()).is_err());
    }

    #[test]
    fn rejects_throw_without_message() {
        assert!(parse_command(&lexer::tokenize("THROW").unwrap()).is_err());
    }

    #[test]
    fn parses_goto_forms() {
        let tokens = lexer::tokenize("GOTO 0").unwrap();
        let (cmd, _) = parse_command(&tokens).unwrap();
        assert_eq!(cmd, Command::Goto(GotoTarget::Id(ValueExpr::Literal(Value::Int(0)))));

        let tokens = lexer::tokenize("GOTO THIS CHECKPOINT").unwrap();
        let (cmd, rest) = parse_command(&tokens).unwrap();
        assert_eq!(cmd, Command::Goto(GotoTarget::ThisCheckpoint));
        assert!(rest.is_empty());
    }

    #[test]
    fn rejects_this_without_checkpoint() {
        assert!(parse_command(&lexer::tokenize("GOTO THIS").unwrap()).is_err());
        assert!(parse_command(&lexer::tokenize("GOTO THIS ROW").unwrap()).is_err());
    }

    #[test]
    fn parses_arithmetic_forms() {
        let tokens = lexer::tokenize("INCREMENT").unwrap();
        let (cmd, _) = parse_command(&tokens).unwrap();
        assert_eq!(cmd, Command::Arithmetic {
            op: ArithOp::Increment, target: None, by: None, giving: None,
        });

        let tokens = lexer::tokenize("INCREMENT x BY 5 GIVING y").unwrap();
        let (cmd, rest) = parse_command(&tokens).unwrap();
        assert_eq!(cmd, Command::Arithmetic {
            op: ArithOp::Increment,
            target: Some(ValueExpr::Var("x".into())),
            by: Some(ValueExpr::Literal(Value::Int(5))),
            giving: Some(ValueExpr::Var("y".into())),
        });
        assert!(rest.is_empty());

        let tokens = lexer::tokenize("INCREMENT BY 5").unwrap();
        let (cmd, _) = parse_command(&tokens).unwrap();
        assert!(matches!(cmd, Command::Arithmetic { target: None, by: Some(_), .. }));
    }

    #[test]
    fn multiply_requires_by() {
        assert!(parse_command(&lexer::tokenize("MULTIPLY x").unwrap()).is_err());
        assert!(parse_command(&lexer::tokenize("MULTIPLY x BY 2").unwrap()).is_ok());
    }

    #[test]
    fn parses_go_forms() {
        let cases = vec![
            ("GO NORTH", GoTarget::North, false),
            ("GO SOUTH", GoTarget::South, false),
            ("GO EAST", GoTarget::East, false),
            ("GO WEST", GoTarget::West, false),
            ("GO RANDOM", GoTarget::Random, false),
            ("GO THIS DIRECTION", GoTarget::ThisDirection, false),
            ("GO RELATIVE TO THIS DIRECTION", GoTarget::ThisDirection, true),
        ];
        for (source, target, relative) in cases {
            let tokens = lexer::tokenize(source).unwrap();
            let (cmd, rest) = parse_command(&tokens).unwrap();
            assert_eq!(cmd, Command::Go { target, relative });
            assert!(rest.is_empty());
        }
    }

    #[test]
    fn parses_go_with_value() {
        let tokens = lexer::tokenize("GO 45").unwrap();
        let (cmd, _) = parse_command(&tokens).unwrap();
        assert_eq!(cmd, Command::Go {
            target: GoTarget::Value(ValueExpr::Literal(Value::Int(45))),
            relative: false,
        });

        let tokens = lexer::tokenize("GO RELATIVE TO 90").unwrap();
        let (cmd, _) = parse_command(&tokens).unwrap();
        assert!(matches!(cmd, Command::Go { relative: true, .. }));
    }

    #[test]
    fn rejects_bad_go_forms() {
        assert!(parse_command(&lexer::tokenize("GO RELATIVE NORTH").unwrap()).is_err());
        assert!(parse_command(&lexer::tokenize("GO RELATIVE TO NORTH").unwrap()).is_err());
        assert!(parse_command(&lexer::tokenize("GO THIS ROW").unwrap()).is_err());
    }

    #[test]
    fn parses_switch_forms() {
        let five = ValueExpr::Literal(Value::Int(5));
        let cases = vec![
            ("SWITCH RANDOM", SwitchCond::Random),
            ("SWITCH 5", SwitchCond::Truthy(five.clone())),
            ("SWITCH !5", SwitchCond::Falsy(five.clone())),
            ("SWITCH =5", SwitchCond::Equals(five.clone())),
            ("SWITCH !=5", SwitchCond::NotEquals(five)),
        ];
        for (source, expected) in cases {
            let tokens = lexer::tokenize(source).unwrap();
            let (cmd, rest) = parse_command(&tokens).unwrap();
            assert_eq!(cmd, Command::Switch(expected));
            assert!(rest.is_empty());
        }
    }

    #[test]
    fn switch_accepts_variables_and_dynamic_names() {
        let tokens = lexer::tokenize("SWITCH !x").unwrap();
        let (cmd, _) = parse_command(&tokens).unwrap();
        assert_eq!(cmd, Command::Switch(SwitchCond::Falsy(ValueExpr::Var("x".into()))));

        let tokens = lexer::tokenize("SWITCH =THE INT NAMED 'n'").unwrap();
        let (cmd, _) = parse_command(&tokens).unwrap();
        assert!(matches!(cmd, Command::Switch(SwitchCond::Equals(ValueExpr::DynamicVar { .. }))));
    }

    #[test]
    fn rejects_switch_without_operand() {
        assert!(parse_command(&lexer::tokenize("SWITCH").unwrap()).is_err());
        assert!(parse_command(&lexer::tokenize("SWITCH =").unwrap()).is_err());
    }

    #[test]
    fn parses_store_forms() {
        let tokens = lexer::tokenize("STORE 5").unwrap();
        let (cmd, _) = parse_command(&tokens).unwrap();
        assert_eq!(cmd, Command::Store {
            source: StoreSource::Value(ValueExpr::Literal(Value::Int(5))),
            target: None,
        });

        let tokens = lexer::tokenize("STORE RANDOM TO x").unwrap();
        let (cmd, rest) = parse_command(&tokens).unwrap();
        assert_eq!(cmd, Command::Store {
            source: StoreSource::Random,
            target: Some(ValueExpr::Var("x".into())),
        });
        assert!(rest.is_empty());
    }
}
