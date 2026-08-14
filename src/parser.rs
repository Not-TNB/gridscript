pub mod ast;
pub mod lexer;

use crate::error::{GridScriptError as Error, Result};
use crate::parser::ast::{
    ArithOp, Command, DebugMode, GoTarget, GotoTarget,
    PrintTarget, RawMetadata, StoreSource, SwitchCond, ToClause,
    ValueExpr, RemoveFrom, MoveMode,
};
use crate::parser::lexer::{Keyword, Token};
use crate::program::Program;
use crate::types::{DataType, Value};

/* ---------------------------------------------------------------------------------------------
 * PARSER STRUCT AND PRIMITIVES
 * --------------------------------------------------------------------------------------------- */

struct Parser<'a> {
    tokens: &'a [Token],
    pos: usize,
}

impl<'a> Parser<'a> {
    fn new(tokens: &'a [Token]) -> Self { Parser { tokens, pos: 0 } }

    /// The token at the cursor, if any.
    fn peek(&self) -> Option<&Token> { self.tokens.get(self.pos) }

    /// The token `n` positions ahead of the cursor, if any.
    fn peek_at(&self, n: usize) -> Option<&Token> { self.tokens.get(self.pos + n) }

    /// Moves the cursor forward one token.
    fn advance(&mut self) { self.pos += 1; }

    /// True if the cursor is past the last token.
    fn at_end(&self) -> bool { self.pos >= self.tokens.len() }

    /// True if the cursor is at a newline or past the end.
    fn at_line_end(&self) -> bool { matches!(self.peek(), None | Some(Token::Newline)) }

    /// The keyword at the cursor, if there is one.
    fn keyword(&self) -> Option<Keyword> {
        match self.peek() {
            Some(Token::Keyword(k)) => Some(*k),
            _ => None,
        }
    }

    /// The keyword `n` positions ahead of the cursor, if there is one.
    fn keyword_at(&self, n: usize) -> Option<Keyword> {
        match self.peek_at(n) {
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
        } else {
            false
        }
    }

    /// Advances past any newlines at the cursor.
    fn skip_newlines(&mut self) {
        while matches!(self.peek(), Some(Token::Newline)) { self.advance(); }
    }

    /// Consumes a type keyword if the cursor is at one.
    fn eat_data_type(&mut self) -> Option<DataType> {
        let ty = match self.keyword() {
            Some(Keyword::Int) => DataType::Int,
            Some(Keyword::Float) => DataType::Float,
            Some(Keyword::Str) => DataType::Str,
            Some(Keyword::Bool) => DataType::Bool,
            _ => return None,
        };
        self.advance();
        Some(ty)
    }
}

/* ---------------------------------------------------------------------------------------------
 * SOURCE PREPROCESSING
 * --------------------------------------------------------------------------------------------- */

/// Splits a leading title line (`#TITLE.` or `##TITLE.`) off the front of `source`,
/// returning the title text and the remaining source.
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

/* ---------------------------------------------------------------------------------------------
 * METADATA PARSING
 * --------------------------------------------------------------------------------------------- */

impl<'a> Parser<'a> {
    /// Reads one integer literal associated with a key, widening it to `i64`.
    fn expect_int(&mut self, key: &str) -> Result<i64> {
        match self.peek() {
            Some(Token::IntLiteral(n)) => {
                let n = *n as i64;
                self.advance();
                Ok(n)
            }
            other => Err(Error::syntax(format!(
                "metadata key '{key}' expects an integer value, found {other:?}"
            ))),
        }
    }

    /// Reads one `true`/`false`/`auto` value associated with `@debug`.
    fn expect_debug_mode(&mut self) -> Result<DebugMode> {
        let mode = match self.peek() {
            Some(Token::Identifier(s)) if s == "true" => DebugMode::True,
            Some(Token::Identifier(s)) if s == "false" => DebugMode::False,
            Some(Token::Identifier(s)) if s == "auto" => DebugMode::Auto,
            other => return Err(Error::syntax(format!(
                "metadata key 'debug' expects true, false, or auto, found {other:?}"
            ))),
        };
        self.advance();
        Ok(mode)
    }

    /// Parses a single `@key value` line into `raw`.
    /// PRE: the cursor is at `Token::At`
    fn parse_metadata_line(&mut self, raw: &mut RawMetadata) -> Result<()> {
        self.advance(); // consume @

        let key = match self.peek() {
            Some(Token::Identifier(name)) => name.clone(),
            other => return Err(Error::syntax(format!(
                "expected a metadata key, found {other:?}"
            ))),
        };
        self.advance();

        macro_rules! int_key {
            ($field:ident) => {{
                raw.$field = Some(self.expect_int(&key)?);
            }};
        }

        match key.as_str() {
            "width"      => int_key!(width),
            "height"     => int_key!(height),
            "datawidth"  => int_key!(data_width),
            "dataheight" => int_key!(data_height),
            "radius"     => int_key!(radius),
            "steps"      => int_key!(steps),
            "maxdepth"   => int_key!(max_depth),
            "seed"  => raw.seed = Some(self.expect_int("seed")? as u64),
            "debug" => raw.debug = Some(self.expect_debug_mode()?),
            _ => return Err(Error::syntax(format!("unknown metadata key '{key}'"))),
        }

        match self.peek() {
            Some(Token::Newline) => self.advance(),
            None => (),
            other => return Err(Error::syntax(format!(
                "expected end of metadata line, found {other:?}"
            ))),
        }

        Ok(())
    }

    /// Parses the whole `@key value` metadata block.
    fn parse_metadata(&mut self) -> Result<RawMetadata> {
        self.skip_newlines();
        let mut raw = RawMetadata::default();
        while matches!(self.peek(), Some(Token::At)) {
            self.parse_metadata_line(&mut raw)?;
        }
        Ok(raw)
    }
}

/* ---------------------------------------------------------------------------------------------
 * VALUE EXPRESSION PARSING
 * --------------------------------------------------------------------------------------------- */

impl<'a> Parser<'a> {
    /// Parses a value expression of the form `THE _ NAMED _`.
    /// PRE: the cursor is at `Keyword::The`
    fn parse_dynamic_value_expr(&mut self) -> Result<ValueExpr> {
        self.advance(); // consume THE
        let cast = if self.eat_keyword(Keyword::Variable) {
            None
        } else if let Some(ty) = self.eat_data_type() {
            Some(ty)
        } else {
            return Err(Error::syntax(format!(
                "expected VARIABLE or a type after THE, found {:?}", self.peek()
            )));
        };
        self.expect_keyword(Keyword::Named)?;
        let name = self.parse_value_expr()?;
        Ok(ValueExpr::DynamicVar { name: Box::new(name), cast })
    }

    /// If the cursor is at `kw`, consumes it and parses the following value
    /// expression. Consumes nothing otherwise.
    fn parse_optional_clause(&mut self, kw: Keyword) -> Result<Option<ValueExpr>> {
        if self.eat_keyword(kw) {
            Ok(Some(self.parse_value_expr()?))
        } else {
            Ok(None)
        }
    }

    /// Parses one value expression: a literal, a variable reference, or a
    /// dynamic `THE _ NAMED _` form.
    fn parse_value_expr(&mut self) -> Result<ValueExpr> {
        let expr = match self.peek() {
            Some(Token::IntLiteral(n)) => ValueExpr::Literal(Value::Int(*n)),
            Some(Token::FloatLiteral(f)) => ValueExpr::Literal(Value::Float(*f)),
            Some(Token::StringLiteral(s)) => ValueExpr::Literal(Value::Str(s.clone())),
            Some(Token::Keyword(Keyword::True)) => ValueExpr::Literal(Value::Bool(true)),
            Some(Token::Keyword(Keyword::False)) => ValueExpr::Literal(Value::Bool(false)),
            Some(Token::Identifier(name)) => ValueExpr::Var(name.clone()),
            Some(Token::Keyword(Keyword::The)) => return self.parse_dynamic_value_expr(),
            other => return Err(Error::syntax(format!(
                "expected a value, found {other:?}"
            ))),
        };
        self.advance();
        Ok(expr)
    }
}

/* ---------------------------------------------------------------------------------------------
 * COMMAND PARSING
 * --------------------------------------------------------------------------------------------- */

impl<'a> Parser<'a> {
    /// Parses either a `VALUE` or a `ROW`, producing a given command.
    /// PRE: the cursor is at `NEXT` or `PREVIOUS`
    fn parse_value_or_row(&mut self, on_value: Command, on_row: Command) -> Result<Command> {
        self.advance(); // consume NEXT/PREVIOUS
        match self.keyword() {
            Some(Keyword::Value) => { self.advance(); Ok(on_value) }
            Some(Keyword::Row)   => { self.advance(); Ok(on_row) }
            _ => Err(Error::syntax(format!(
                "expected VALUE or ROW, found {:?}", self.peek()
            ))),
        }
    }

    /// Parses `PUSH [value]`.
    fn parse_push(&mut self) -> Result<Command> {
        self.advance(); // consume PUSH
        Ok(Command::Push(
            if self.at_line_end() { None }
            else { Some(self.parse_value_expr()?) }
        ))
    }

    /// Parses `GOTO id|THIS CHECKPOINT`.
    fn parse_goto(&mut self) -> Result<Command> {
        self.advance(); // consume GOTO
        Ok(Command::Goto(
            if self.eat_keyword(Keyword::This) {
                self.expect_keyword(Keyword::Checkpoint)?;
                GotoTarget::ThisCheckpoint
            } else {
                GotoTarget::Id(self.parse_value_expr()?)
            }
        ))
    }

    /// Parses `INCREMENT|DECREMENT|MULTIPLY|DIVIDE [var] [BY value] [GIVING var]`.
    fn parse_arithmetic(&mut self, op: ArithOp) -> Result<Command> {
        self.advance(); // consume the operator keyword

        let target = if self.at_keyword(Keyword::By)
            || self.at_keyword(Keyword::Giving)
            || self.at_line_end()
        { None } else {
            Some(self.parse_value_expr()?)
        };

        let by = self.parse_optional_clause(Keyword::By)?;
        let giving = self.parse_optional_clause(Keyword::Giving)?;

        if by.is_none() && matches!(op, ArithOp::Multiply | ArithOp::Divide) {
            return Err(Error::syntax(format!("{op:?} requires a BY clause")));
        }

        Ok(Command::Arithmetic { op, target, by, giving })
    }

    /// Parses `THROW message`.
    fn parse_throw(&mut self) -> Result<Command> {
        self.advance(); Ok(Command::Throw(self.parse_value_expr()?))
    }

    /// Parses `WARN message`.
    fn parse_warn(&mut self) -> Result<Command> {
        self.advance(); Ok(Command::Warn(self.parse_value_expr()?))
    }

    /// Parses `GO NORTH|SOUTH|EAST|WEST|RANDOM|[RELATIVE TO] THIS DIRECTION|[RELATIVE TO] value`.
    fn parse_go(&mut self) -> Result<Command> {
        self.advance(); // consume GO

        let relative = if self.eat_keyword(Keyword::Relative) {
            self.expect_keyword(Keyword::To)?;
            true
        } else {
            false
        };

        let target = match self.keyword() {
            Some(Keyword::North)  => { self.advance(); GoTarget::North }
            Some(Keyword::South)  => { self.advance(); GoTarget::South }
            Some(Keyword::East)   => { self.advance(); GoTarget::East }
            Some(Keyword::West)   => { self.advance(); GoTarget::West }
            Some(Keyword::Random) => { self.advance(); GoTarget::Random }
            Some(Keyword::This) => {
                self.advance();
                self.expect_keyword(Keyword::Direction)?;
                GoTarget::ThisDirection
            }
            _ => GoTarget::Value(self.parse_value_expr()?),
        };

        if relative && !matches!(target, GoTarget::ThisDirection | GoTarget::Value(_)) {
            return Err(Error::syntax(format!(
                "cannot make {target:?} a relative target"
            )));
        }

        Ok(Command::Go { target, relative })
    }

    /// Parses `SWITCH RANDOM|value|!value|=value|!=value`.
    fn parse_switch(&mut self) -> Result<Command> {
        self.advance(); // consume SWITCH

        if self.eat_keyword(Keyword::Random) {
            return Ok(Command::Switch(SwitchCond::Random));
        }

        let build: fn(ValueExpr) -> SwitchCond = match self.peek() {
            Some(Token::Bang)       => { self.advance(); SwitchCond::Falsy }
            Some(Token::Equals)     => { self.advance(); SwitchCond::Equals }
            Some(Token::BangEquals) => { self.advance(); SwitchCond::NotEquals }
            _ => SwitchCond::Truthy,
        };

        Ok(Command::Switch(build(self.parse_value_expr()?)))
    }

    /// Parses `STORE value|RANDOM [TO variable]`.
    fn parse_store(&mut self) -> Result<Command> {
        self.advance(); // consume STORE

        let source = if self.eat_keyword(Keyword::Random) {
            StoreSource::Random
        } else {
            StoreSource::Value(self.parse_value_expr()?)
        };

        let target = self.parse_optional_clause(Keyword::To)?;
        Ok(Command::Store { source, target })
    }

    /// Parses `TO [type] variable` clause.
    fn parse_to_clause(&mut self) -> Result<Option<ToClause>> {
        if !self.eat_keyword(Keyword::To) { return Ok(None); }
        let cast = self.eat_data_type();
        let target = self.parse_value_expr()?;
        Ok(Some(ToClause { cast , target }))
    }

    /// Parses `PEEK [TO [type] VARIABLE]`.
    fn parse_peek(&mut self) -> Result<Command> {
        self.advance(); Ok(Command::Peek(self.parse_to_clause()?))
    }

    /// Parses `SPLIT string [OVER separator]`
    fn parse_split(&mut self) -> Result<Command> {
        self.advance(); // consume SPLIT
        let value = self.parse_value_expr()?;
        let over = self.parse_optional_clause(Keyword::Over)?;
        Ok(Command::Split { value, over })
    }

    /// Parses `RETURN [value]`
    fn parse_return(&mut self) -> Result<Command> {
        self.advance(); // consume RETURN
        Ok(Command::Return(
            if self.at_line_end() { None }
            else { Some(self.parse_value_expr()?) }
        ))
    }

    /// Parses `PRINT [value|NEWLINE|IMAGE path|FILE path]`
    fn parse_print(&mut self) -> Result<Command> {
        self.advance(); // consume PRINT
        Ok(Command::Print(
            if self.at_line_end() { PrintTarget::DataCell }
            else {
                match self.keyword() {
                    Some(Keyword::Newline) => { self.advance();
                        PrintTarget::Newline
                    }
                    Some(Keyword::Image) => { self.advance();
                        PrintTarget::Image(self.parse_value_expr()?)
                    }
                    Some(Keyword::File) => { self.advance();
                        PrintTarget::File(self.parse_value_expr()?)
                    }
                    _ => PrintTarget::Value(self.parse_value_expr()?),
                }
            }
        ))
    }

    /// Parses `REMOVE [position|THIS POSITION|TOP|ANY POSITION] [TO [type] variable]`
    fn parse_remove(&mut self) -> Result<Command> {
        self.advance(); // consume REMOVE
        let from = match self.keyword() {
            Some(Keyword::Top) => { self.advance();
                RemoveFrom::Top
            }
            Some(Keyword::Any) => { self.advance();
                self.expect_keyword(Keyword::Position)?;
                RemoveFrom::AnyPosition
            }
            Some(Keyword::This) => { self.advance();
                self.expect_keyword(Keyword::Position)?;
                RemoveFrom::ThisPosition
            }
            _ if self.at_line_end() || self.at_keyword(Keyword::To) => RemoveFrom::Bottom,
            _ => RemoveFrom::Position(self.parse_value_expr()?),
        };
        let to = self.parse_to_clause()?;
        Ok(Command::Remove { from, to })
    }

    /// Parses `LOAD FILE path [TO [type] variable]`
    fn parse_load_file(&mut self) -> Result<Command> {
        self.advance();
        self.expect_keyword(Keyword::File)?;
        let path = self.parse_value_expr()?;
        let to = self.parse_to_clause()?;
        Ok(Command::LoadFile { path, to })
    }

    /// Parses `MOVE LAST NODE TO|BY x y`
    fn parse_move_last_node(&mut self) -> Result<Command> {
        self.advance();
        self.expect_keyword(Keyword::Last)?;
        self.expect_keyword(Keyword::Node)?;
        let mode = if self.eat_keyword(Keyword::To) {
            MoveMode::To
        } else if self.eat_keyword(Keyword::By) {
            MoveMode::By
        } else {
            return Err(Error::syntax(format!(
                "expected TO or BY after MOVE LAST NODE, found {:?}", self.peek()
            )));
        };
        let x = self.parse_value_expr()?;
        let y = self.parse_value_expr()?;
        Ok(Command::MoveLastNode { mode, x, y })
    }

    /// Parses `CALL name [WITH ARGUMENTS args] [GIVING variable]`
    fn parse_call(&mut self) -> Result<Command> {
        self.advance();
        let name = match self.peek() {
            Some(Token::UpperName(name)) => name.clone(),
            Some(Token::Keyword(kw)) => return Err(Error::syntax(format!(
                "subroutine name cannot be a reserved word: {kw:?}"
            ))),
            other => return Err(Error::syntax(format!(
                "expected a subroutine name, found {other:?}"
            ))),
        };

        self.advance();
        let mut arguments: Vec<ValueExpr> = Vec::new();
        if self.eat_keyword(Keyword::With) {
            self.expect_keyword(Keyword::Arguments)?;
            while !self.at_line_end() && !self.at_keyword(Keyword::Giving) {
                arguments.push(self.parse_value_expr()?);
            }
        }

        let giving = self.parse_optional_clause(Keyword::Giving)?;

        Ok(Command::Call { name, giving, arguments })
    }

    /// Parses one command.
    fn parse_command(&mut self) -> Result<Command> {
        match self.keyword() {
            Some(Keyword::Start)   => { self.advance(); Ok(Command::Start) }
            Some(Keyword::Home)    => { self.advance(); Ok(Command::Home) }
            Some(Keyword::Shuffle) => { self.advance(); Ok(Command::Shuffle) }

            Some(Keyword::Next) =>
                self.parse_value_or_row(Command::NextValue, Command::NextRow),
            Some(Keyword::Previous) =>
                self.parse_value_or_row(Command::PreviousValue, Command::PreviousRow),

            Some(Keyword::Increment) => self.parse_arithmetic(ArithOp::Increment),
            Some(Keyword::Decrement) => self.parse_arithmetic(ArithOp::Decrement),
            Some(Keyword::Multiply)  => self.parse_arithmetic(ArithOp::Multiply),
            Some(Keyword::Divide)    => self.parse_arithmetic(ArithOp::Divide),
            Some(Keyword::Throw)     => self.parse_throw(),
            Some(Keyword::Warn)      => self.parse_warn(),
            Some(Keyword::Push)      => self.parse_push(),
            Some(Keyword::Goto)      => self.parse_goto(),
            Some(Keyword::Go)        => self.parse_go(),
            Some(Keyword::Switch)    => self.parse_switch(),
            Some(Keyword::Store)     => self.parse_store(),
            Some(Keyword::Peek)      => self.parse_peek(),
            Some(Keyword::Split)     => self.parse_split(),
            Some(Keyword::Return)    => self.parse_return(),
            Some(Keyword::Print)     => self.parse_print(),
            Some(Keyword::Remove)    => self.parse_remove(),
            Some(Keyword::Load)      => self.parse_load_file(),
            Some(Keyword::Move)      => self.parse_move_last_node(),
            Some(Keyword::Call)      => self.parse_call(),

            _ => Err(Error::syntax(format!(
                "expected a command, found {:?}", self.peek()
            ))),
        }
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

    /// Parses a source snippet as a command.
    fn cmd(source: &str) -> Result<Command> {
        let tokens = lexer::tokenize(source)?;
        Parser::new(&tokens).parse_command()
    }

    /// Parses a source snippet as a value expression.
    fn expr(source: &str) -> Result<ValueExpr> {
        let tokens = lexer::tokenize(source)?;
        Parser::new(&tokens).parse_value_expr()
    }

    /// Parses a source snippet as a metadata block.
    fn meta(source: &str) -> Result<RawMetadata> {
        let tokens = lexer::tokenize(source)?;
        Parser::new(&tokens).parse_metadata()
    }

    fn int(n: i32) -> ValueExpr { ValueExpr::Literal(Value::Int(n)) }
    fn var(s: &str) -> ValueExpr { ValueExpr::Var(s.into()) }
    fn str_lit(s: &str) -> ValueExpr { ValueExpr::Literal(Value::Str(s.as_bytes().to_vec())) }

    /* --- titles --- */

    #[test]
    fn splits_titles() {
        let (title, rest) = split_title("#HELLO WORLD.\n\n@width 4", 1).unwrap();
        assert_eq!(title, "HELLO WORLD");
        assert_eq!(rest, "\n@width 4");

        assert!(split_title("##ACK.\n", 1).is_err());
        assert!(split_title("#NO PERIOD\n", 1).is_err());
    }

    /* --- metadata --- */

    #[test]
    fn parses_metadata_block() {
        let raw = meta("\n@width 4\n@height 1\n@debug auto\n").unwrap();
        assert_eq!(raw.width, Some(4));
        assert_eq!(raw.height, Some(1));
        assert_eq!(raw.debug, Some(DebugMode::Auto));

        assert!(meta("@bogus 5\n").is_err());
    }

    /* --- value expressions --- */

    #[test]
    fn parses_literals_and_variables() {
        assert_eq!(expr("42").unwrap(), int(42));
        assert_eq!(expr("-7").unwrap(), int(-7));
        assert_eq!(expr("3.5").unwrap(), ValueExpr::Literal(Value::Float(3.5)));
        assert_eq!(expr("'hi'").unwrap(), str_lit("hi"));
        assert_eq!(expr("TRUE").unwrap(), ValueExpr::Literal(Value::Bool(true)));
        assert_eq!(expr("count_2").unwrap(), var("count_2"));
    }

    #[test]
    fn parses_dynamic_variables() {
        assert_eq!(expr("THE VARIABLE NAMED 'x'").unwrap(), ValueExpr::DynamicVar {
            name: Box::new(str_lit("x")),
            cast: None,
        });
        assert_eq!(expr("THE INT NAMED holder").unwrap(), ValueExpr::DynamicVar {
            name: Box::new(var("holder")),
            cast: Some(DataType::Int),
        });
        // nesting exercises the recursion and the Box
        assert_eq!(expr("THE VARIABLE NAMED THE STRING NAMED 'outer'").unwrap(),
                   ValueExpr::DynamicVar {
                       name: Box::new(ValueExpr::DynamicVar {
                           name: Box::new(str_lit("outer")),
                           cast: Some(DataType::Str),
                       }),
                       cast: None,
                   });
    }

    #[test]
    fn rejects_malformed_value_expressions() {
        assert!(expr("THE VARIABLE 'x'").is_err()); // missing NAMED
        assert!(expr("THE NAMED 'x'").is_err());    // bad slot after THE
        assert!(expr("THE").is_err());              // nothing after THE
        assert!(expr(":").is_err());
        assert!(expr("").is_err());
    }

    /* --- commands --- */

    #[test]
    fn parses_zero_argument_commands() {
        let cases = [
            ("START", Command::Start),
            ("HOME", Command::Home),
            ("SHUFFLE", Command::Shuffle),
            ("NEXT VALUE", Command::NextValue),
            ("NEXT ROW", Command::NextRow),
            ("PREVIOUS VALUE", Command::PreviousValue),
            ("PREVIOUS ROW", Command::PreviousRow),
        ];
        for (source, expected) in cases {
            assert_eq!(cmd(source).unwrap(), expected);
        }
        assert!(cmd("NEXT").is_err());
        assert!(cmd("NEXT HOME").is_err());
    }

    #[test]
    fn parses_single_value_commands() {
        assert_eq!(cmd("THROW 'boom'").unwrap(), Command::Throw(str_lit("boom")));
        assert_eq!(cmd("WARN x").unwrap(), Command::Warn(var("x")));
        assert_eq!(cmd("PUSH 5").unwrap(), Command::Push(Some(int(5))));
        assert_eq!(cmd("PUSH").unwrap(), Command::Push(None));
        assert!(cmd("THROW").is_err());
    }

    #[test]
    fn parses_goto() {
        assert_eq!(cmd("GOTO 0").unwrap(), Command::Goto(GotoTarget::Id(int(0))));
        assert_eq!(cmd("GOTO THIS CHECKPOINT").unwrap(),
                   Command::Goto(GotoTarget::ThisCheckpoint));
        assert!(cmd("GOTO THIS").is_err());
        assert!(cmd("GOTO THIS ROW").is_err());
    }

    #[test]
    fn parses_arithmetic() {
        assert_eq!(cmd("INCREMENT").unwrap(), Command::Arithmetic {
            op: ArithOp::Increment, target: None, by: None, giving: None,
        });
        assert_eq!(cmd("INCREMENT x BY 5 GIVING y").unwrap(), Command::Arithmetic {
            op: ArithOp::Increment,
            target: Some(var("x")),
            by: Some(int(5)),
            giving: Some(var("y")),
        });
        // BY in the target slot must not be eaten as a target
        assert!(matches!(cmd("INCREMENT BY 5").unwrap(),
            Command::Arithmetic { target: None, by: Some(_), .. }));

        assert!(cmd("MULTIPLY x").is_err());
        assert!(cmd("MULTIPLY x BY 2").is_ok());
    }

    #[test]
    fn parses_go() {
        let cases = [
            ("GO NORTH", GoTarget::North, false),
            ("GO SOUTH", GoTarget::South, false),
            ("GO EAST", GoTarget::East, false),
            ("GO WEST", GoTarget::West, false),
            ("GO RANDOM", GoTarget::Random, false),
            ("GO THIS DIRECTION", GoTarget::ThisDirection, false),
            ("GO RELATIVE TO THIS DIRECTION", GoTarget::ThisDirection, true),
            ("GO 45", GoTarget::Value(int(45)), false),
            ("GO RELATIVE TO 90", GoTarget::Value(int(90)), true),
        ];
        for (source, target, relative) in cases {
            assert_eq!(cmd(source).unwrap(), Command::Go { target, relative });
        }

        assert!(cmd("GO RELATIVE NORTH").is_err());
        assert!(cmd("GO RELATIVE TO NORTH").is_err());
        assert!(cmd("GO THIS ROW").is_err());
    }

    #[test]
    fn parses_switch() {
        let cases = [
            ("SWITCH RANDOM", SwitchCond::Random),
            ("SWITCH 5", SwitchCond::Truthy(int(5))),
            ("SWITCH !5", SwitchCond::Falsy(int(5))),
            ("SWITCH =5", SwitchCond::Equals(int(5))),
            ("SWITCH !=5", SwitchCond::NotEquals(int(5))),
        ];
        for (source, expected) in cases {
            assert_eq!(cmd(source).unwrap(), Command::Switch(expected));
        }
        // operator forms compose with the full value grammar
        assert!(matches!(cmd("SWITCH =THE INT NAMED 'n'").unwrap(),
            Command::Switch(SwitchCond::Equals(ValueExpr::DynamicVar { .. }))));

        assert!(cmd("SWITCH").is_err());
        assert!(cmd("SWITCH =").is_err());
    }

    #[test]
    fn parses_store() {
        assert_eq!(cmd("STORE 5").unwrap(), Command::Store {
            source: StoreSource::Value(int(5)),
            target: None,
        });
        assert_eq!(cmd("STORE RANDOM TO x").unwrap(), Command::Store {
            source: StoreSource::Random,
            target: Some(var("x")),
        });
    }

    /* --- cursor behaviour --- */

    #[test]
    fn stops_at_line_end_without_consuming() {
        let tokens = lexer::tokenize("PUSH\nHOME").unwrap();
        let mut p = Parser::new(&tokens);
        assert_eq!(p.parse_command().unwrap(), Command::Push(None));
        assert_eq!(p.peek(), Some(&Token::Newline));
    }

    #[test]
    fn leaves_trailing_tokens_unconsumed() {
        let tokens = lexer::tokenize("5 GIVING x").unwrap();
        let mut p = Parser::new(&tokens);
        assert_eq!(p.parse_value_expr().unwrap(), int(5));
        assert_eq!(p.peek(), Some(&Token::Keyword(Keyword::Giving)));
    }

    #[test]
    fn parses_peek() {
        assert_eq!(cmd("PEEK").unwrap(), Command::Peek(None));
        assert_eq!(cmd("PEEK TO x").unwrap(), Command::Peek(Some(ToClause {
            cast: None,
            target: var("x"),
        })));
        assert_eq!(cmd("PEEK TO INT x").unwrap(), Command::Peek(Some(ToClause {
            cast: Some(DataType::Int),
            target: var("x"),
        })));
        assert!(cmd("PEEK TO").is_err());
    }

    #[test]
    fn parses_split_and_return() {
        assert_eq!(cmd("SPLIT 'a b'").unwrap(), Command::Split {
            value: str_lit("a b"), over: None,
        });
        assert_eq!(cmd("SPLIT s OVER ','").unwrap(), Command::Split {
            value: var("s"), over: Some(str_lit(",")),
        });
        assert_eq!(cmd("RETURN").unwrap(), Command::Return(None));
        assert_eq!(cmd("RETURN 5").unwrap(), Command::Return(Some(int(5))));
        assert!(cmd("SPLIT").is_err());
    }

    #[test]
    fn parses_print() {
        assert_eq!(cmd("PRINT").unwrap(), Command::Print(PrintTarget::DataCell));
        assert_eq!(cmd("PRINT NEWLINE").unwrap(), Command::Print(PrintTarget::Newline));
        assert_eq!(cmd("PRINT 'hi'").unwrap(), Command::Print(PrintTarget::Value(str_lit("hi"))));
        assert_eq!(cmd("PRINT FILE 'a.txt'").unwrap(),
                   Command::Print(PrintTarget::File(str_lit("a.txt"))));
        assert_eq!(cmd("PRINT IMAGE p").unwrap(),
                   Command::Print(PrintTarget::Image(var("p"))));
    }

    #[test]
    fn parses_remove() {
        assert_eq!(cmd("REMOVE").unwrap(),
                   Command::Remove { from: RemoveFrom::Bottom, to: None });
        assert_eq!(cmd("REMOVE TOP").unwrap(),
                   Command::Remove { from: RemoveFrom::Top, to: None });
        assert_eq!(cmd("REMOVE ANY POSITION").unwrap(),
                   Command::Remove { from: RemoveFrom::AnyPosition, to: None });
        assert_eq!(cmd("REMOVE THIS POSITION").unwrap(),
                   Command::Remove { from: RemoveFrom::ThisPosition, to: None });
        assert_eq!(cmd("REMOVE 3").unwrap(),
                   Command::Remove { from: RemoveFrom::Position(int(3)), to: None });

        // bare REMOVE followed directly by TO -- the guard's reason for existing
        assert_eq!(cmd("REMOVE TO INT x").unwrap(), Command::Remove {
            from: RemoveFrom::Bottom,
            to: Some(ToClause { cast: Some(DataType::Int), target: var("x") }),
        });

        assert!(cmd("REMOVE ANY").is_err());
        assert!(cmd("REMOVE THIS ROW").is_err());
    }

    #[test]
    fn parses_load_file() {
        assert_eq!(cmd("LOAD FILE 'a.txt'").unwrap(), Command::LoadFile {
            path: str_lit("a.txt"), to: None,
        });
        assert_eq!(cmd("LOAD FILE p TO STRING s").unwrap(), Command::LoadFile {
            path: var("p"),
            to: Some(ToClause { cast: Some(DataType::Str), target: var("s") }),
        });

        assert!(cmd("LOAD 'a.txt'").is_err()); // missing FILE
        assert!(cmd("LOAD FILE").is_err());
    }

    #[test]
    fn parses_move_last_node() {
        assert_eq!(cmd("MOVE LAST NODE TO 3 4").unwrap(), Command::MoveLastNode {
            mode: MoveMode::To, x: int(3), y: int(4),
        });
        assert_eq!(cmd("MOVE LAST NODE BY -1 2").unwrap(), Command::MoveLastNode {
            mode: MoveMode::By, x: int(-1), y: int(2),
        });

        assert!(cmd("MOVE LAST NODE 3 4").is_err());  // missing TO/BY
        assert!(cmd("MOVE NODE TO 3 4").is_err());    // missing LAST
        assert!(cmd("MOVE LAST NODE TO 3").is_err()); // missing y
    }

    #[test]
    fn parses_call() {
        assert_eq!(cmd("CALL ACK").unwrap(), Command::Call {
            name: "ACK".into(), arguments: vec![], giving: None,
        });
        assert_eq!(cmd("CALL ACK WITH ARGUMENTS x y GIVING z").unwrap(), Command::Call {
            name: "ACK".into(),
            arguments: vec![var("x"), var("y")],
            giving: Some(var("z")),
        });
        assert_eq!(cmd("CALL FOO WITH ARGUMENTS 'Foo' 17").unwrap(), Command::Call {
            name: "FOO".into(),
            arguments: vec![str_lit("Foo"), int(17)],
            giving: None,
        });
        assert_eq!(cmd("CALL FOO GIVING x").unwrap(), Command::Call {
            name: "FOO".into(), arguments: vec![], giving: Some(var("x")),
        });

        // a multi-token argument counts as one argument, not four
        assert_eq!(cmd("CALL FOO WITH ARGUMENTS THE INT NAMED 'n' 5").unwrap(), Command::Call {
            name: "FOO".into(),
            arguments: vec![
                ValueExpr::DynamicVar {
                    name: Box::new(str_lit("n")),
                    cast: Some(DataType::Int),
                },
                int(5),
            ],
            giving: None,
        });

        assert!(cmd("CALL PRINT").is_err()); // reserved word as name
        assert!(cmd("CALL x").is_err());     // lowercase name
        assert!(cmd("CALL").is_err());
        assert!(cmd("CALL FOO WITH x").is_err()); // missing ARGUMENTS
    }

    #[test]
    fn call_arguments_stop_at_line_end() {
        let tokens = lexer::tokenize("CALL FOO WITH ARGUMENTS x\nHOME").unwrap();
        let mut p = Parser::new(&tokens);
        assert_eq!(p.parse_command().unwrap(), Command::Call {
            name: "FOO".into(), arguments: vec![var("x")], giving: None,
        });
        assert_eq!(p.peek(), Some(&Token::Newline));
    }
}
