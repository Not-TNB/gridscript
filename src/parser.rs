pub mod ast;
pub mod lexer;

use crate::error::{GridScriptError as Error, Result};
use crate::parser::ast::{
    ArithOp, Checkpoint, Command, DebugMode, GoTarget, GotoTarget, MoveMode, Node, Position,
    PrintTarget, RawMetadata, RemoveFrom, StoreSource, SwitchCond, ToClause, ValueExpr,
};
use crate::parser::lexer::{Keyword, Token};
use crate::program::{Metadata, Program, Scope};
use crate::types::{DataType, Value};
use std::collections::HashMap;

/* ----------------------------------------------------------------------------------------------
 * PARSER STRUCT AND PRIMITIVES
 * ---------------------------------------------------------------------------------------------- */

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

    /// Moves the cursor forward one token.
    fn advance(&mut self) {
        self.pos += 1;
    }

    /// True if the cursor is past the last token.
    fn at_end(&self) -> bool {
        self.pos >= self.tokens.len()
    }

    /// True if the cursor is at a newline or past the end.
    fn at_line_end(&self) -> bool {
        matches!(self.peek(), None | Some(Token::Newline))
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
            Err(Error::syntax(format!(
                "expected {kw:?}, found {:?}",
                self.peek()
            )))
        }
    }

    /// Reads one integer literal, widening it to `i64`.
    fn expect_int(&mut self) -> Result<i64> {
        match self.peek() {
            Some(Token::IntLiteral(n)) => {
                let n = i64::from(*n);
                self.advance();
                Ok(n)
            }
            other => Err(Error::syntax(format!(
                "expected an integer, found {other:?}"
            ))),
        }
    }

    /// Consumes the given token, erroring if it isn't there.
    fn expect_token(&mut self, expected: Token) -> Result<()> {
        if self.peek() == Some(&expected) {
            self.advance();
            Ok(())
        } else {
            Err(Error::syntax(format!(
                "expected {expected:?}, found {:?}",
                self.peek()
            )))
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
        while matches!(self.peek(), Some(Token::Newline)) {
            self.advance();
        }
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

/* ----------------------------------------------------------------------------------------------
 * SOURCE PREPROCESSING
 * ---------------------------------------------------------------------------------------------- */

/// Splits a leading title line (`#TITLE.` or `##TITLE.`) off the front of `source`,
/// returning the title text and the remaining source.
fn split_title(source: &str, octothorpes: usize) -> Result<(&str, &str)> {
    let (line, rest) = match source.split_once('\n') {
        Some((line, rest)) => (line, rest),
        None => (source, ""),
    };

    let line = line.trim();

    let prefix = "#".repeat(octothorpes);
    let body = line.strip_prefix(&prefix).ok_or_else(|| {
        Error::syntax(format!(
            "expected a title line beginning with '{prefix}', found '{line}'"
        ))
    })?;

    if body.starts_with('#') {
        return Err(Error::syntax(format!(
            "too many leading octothorpes in title line '{line}'"
        )));
    }

    let title = body.strip_suffix('.').ok_or_else(|| {
        Error::syntax(format!("title line must end with a period, found '{line}'"))
    })?;

    Ok((title.trim(), rest))
}

/* ----------------------------------------------------------------------------------------------
 * METADATA PARSING
 * ---------------------------------------------------------------------------------------------- */

impl<'a> Parser<'a> {
    /// Reads one `true`/`false`/`auto` value associated with `@debug`.
    fn expect_debug_mode(&mut self) -> Result<DebugMode> {
        let mode = match self.peek() {
            Some(Token::Identifier(s)) if s == "true" => DebugMode::True,
            Some(Token::Identifier(s)) if s == "false" => DebugMode::False,
            Some(Token::Identifier(s)) if s == "auto" => DebugMode::Auto,
            other => {
                return Err(Error::syntax(format!(
                    "metadata key 'debug' expects true, false, or auto, found {other:?}"
                )));
            }
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
            other => {
                return Err(Error::syntax(format!(
                    "expected a metadata key, found {other:?}"
                )));
            }
        };
        self.advance();

        match key.as_str() {
            "width" => raw.width = Some(self.expect_int()?),
            "height" => raw.height = Some(self.expect_int()?),
            "datawidth" => raw.data_width = Some(self.expect_int()?),
            "dataheight" => raw.data_height = Some(self.expect_int()?),
            "radius" => raw.radius = Some(self.expect_int()?),
            "steps" => raw.steps = Some(self.expect_int()?),
            "maxdepth" => raw.max_depth = Some(self.expect_int()?),
            "seed" => raw.seed = Some(self.expect_int()? as u64),
            "debug" => raw.debug = Some(self.expect_debug_mode()?),
            _ => return Err(Error::syntax(format!("unknown metadata key '{key}'"))),
        }

        match self.peek() {
            Some(Token::Newline) => self.advance(),
            None => (),
            other => {
                return Err(Error::syntax(format!(
                    "expected end of metadata line, found {other:?}"
                )));
            }
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

/* ----------------------------------------------------------------------------------------------
 * VALUE EXPRESSION PARSING
 * ---------------------------------------------------------------------------------------------- */

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
                "expected VARIABLE or a type after THE, found {:?}",
                self.peek()
            )));
        };
        self.expect_keyword(Keyword::Named)?;
        let name = self.parse_value_expr()?;
        Ok(ValueExpr::DynamicVar {
            name: Box::new(name),
            cast,
        })
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
            other => return Err(Error::syntax(format!("expected a value, found {other:?}"))),
        };
        self.advance();
        Ok(expr)
    }
}

/* ----------------------------------------------------------------------------------------------
 * COMMAND PARSING
 * ---------------------------------------------------------------------------------------------- */

impl<'a> Parser<'a> {
    /// Parses one command.
    fn parse_command(&mut self) -> Result<Command> {
        match self.keyword() {
            Some(Keyword::Start) => {
                self.advance();
                Ok(Command::Start)
            }
            Some(Keyword::Home) => {
                self.advance();
                Ok(Command::Home)
            }
            Some(Keyword::Shuffle) => {
                self.advance();
                Ok(Command::Shuffle)
            }

            Some(Keyword::Next) => self.parse_value_or_row(Command::NextValue, Command::NextRow),
            Some(Keyword::Previous) => {
                self.parse_value_or_row(Command::PreviousValue, Command::PreviousRow)
            }

            Some(Keyword::Increment) => self.parse_arithmetic(ArithOp::Increment),
            Some(Keyword::Decrement) => self.parse_arithmetic(ArithOp::Decrement),
            Some(Keyword::Multiply) => self.parse_arithmetic(ArithOp::Multiply),
            Some(Keyword::Divide) => self.parse_arithmetic(ArithOp::Divide),
            Some(Keyword::Throw) => self.parse_throw(),
            Some(Keyword::Warn) => self.parse_warn(),
            Some(Keyword::Push) => self.parse_push(),
            Some(Keyword::Goto) => self.parse_goto(),
            Some(Keyword::Go) => self.parse_go(),
            Some(Keyword::Switch) => self.parse_switch(),
            Some(Keyword::Store) => self.parse_store(),
            Some(Keyword::Peek) => self.parse_peek(),
            Some(Keyword::Split) => self.parse_split(),
            Some(Keyword::Return) => self.parse_return(),
            Some(Keyword::Print) => self.parse_print(),
            Some(Keyword::Remove) => self.parse_remove(),
            Some(Keyword::Load) => self.parse_load_file(),
            Some(Keyword::Move) => self.parse_move_last_node(),
            Some(Keyword::Call) => self.parse_call(),
            Some(Keyword::Input) => self.parse_input(),

            _ => Err(Error::syntax(format!(
                "expected a command, found {:?}",
                self.peek()
            ))),
        }
    }

    /// Parses either a `VALUE` or a `ROW`, producing a given command.
    /// PRE: the cursor is at `NEXT` or `PREVIOUS`.
    fn parse_value_or_row(&mut self, on_value: Command, on_row: Command) -> Result<Command> {
        self.advance(); // consume NEXT/PREVIOUS
        match self.keyword() {
            Some(Keyword::Value) => {
                self.advance();
                Ok(on_value)
            }
            Some(Keyword::Row) => {
                self.advance();
                Ok(on_row)
            }
            _ => Err(Error::syntax(format!(
                "expected VALUE or ROW, found {:?}",
                self.peek()
            ))),
        }
    }

    /// Parses `PUSH [value]`.
    fn parse_push(&mut self) -> Result<Command> {
        self.advance(); // consume PUSH
        Ok(Command::Push(if self.at_line_end() {
            None
        } else {
            Some(self.parse_value_expr()?)
        }))
    }

    /// Parses `GOTO id|THIS CHECKPOINT`.
    fn parse_goto(&mut self) -> Result<Command> {
        self.advance(); // consume GOTO
        Ok(Command::Goto(if self.eat_keyword(Keyword::This) {
            self.expect_keyword(Keyword::Checkpoint)?;
            GotoTarget::ThisCheckpoint
        } else {
            GotoTarget::Id(self.parse_value_expr()?)
        }))
    }

    /// Parses `INCREMENT|DECREMENT|MULTIPLY|DIVIDE [var] [BY value] [GIVING var]`.
    fn parse_arithmetic(&mut self, op: ArithOp) -> Result<Command> {
        self.advance(); // consume the operator keyword

        let target = if self.at_keyword(Keyword::By)
            || self.at_keyword(Keyword::Giving)
            || self.at_line_end()
        {
            None
        } else {
            Some(self.parse_value_expr()?)
        };

        let by = self.parse_optional_clause(Keyword::By)?;
        let giving = self.parse_optional_clause(Keyword::Giving)?;

        if by.is_none() && matches!(op, ArithOp::Multiply | ArithOp::Divide) {
            return Err(Error::syntax(format!("{op:?} requires a BY clause")));
        }

        Ok(Command::Arithmetic {
            op,
            target,
            by,
            giving,
        })
    }

    /// Parses `THROW message`.
    fn parse_throw(&mut self) -> Result<Command> {
        self.advance();
        Ok(Command::Throw(self.parse_value_expr()?))
    }

    /// Parses `WARN message`.
    fn parse_warn(&mut self) -> Result<Command> {
        self.advance();
        Ok(Command::Warn(self.parse_value_expr()?))
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
            Some(Keyword::North) => {
                self.advance();
                GoTarget::North
            }
            Some(Keyword::South) => {
                self.advance();
                GoTarget::South
            }
            Some(Keyword::East) => {
                self.advance();
                GoTarget::East
            }
            Some(Keyword::West) => {
                self.advance();
                GoTarget::West
            }
            Some(Keyword::Random) => {
                self.advance();
                GoTarget::Random
            }
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
            Some(Token::Bang) => {
                self.advance();
                SwitchCond::Falsy
            }
            Some(Token::Equals) => {
                self.advance();
                SwitchCond::Equals
            }
            Some(Token::BangEquals) => {
                self.advance();
                SwitchCond::NotEquals
            }
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
        if !self.eat_keyword(Keyword::To) {
            return Ok(None);
        }
        let cast = self.eat_data_type();
        let target = self.parse_value_expr()?;
        Ok(Some(ToClause { cast, target }))
    }

    /// Parses `PEEK [TO [type] VARIABLE]`.
    fn parse_peek(&mut self) -> Result<Command> {
        self.advance();
        Ok(Command::Peek(self.parse_to_clause()?))
    }

    /// Parses `SPLIT string [OVER separator]`.
    fn parse_split(&mut self) -> Result<Command> {
        self.advance(); // consume SPLIT
        let value = self.parse_value_expr()?;
        let over = self.parse_optional_clause(Keyword::Over)?;
        Ok(Command::Split { value, over })
    }

    /// Parses `RETURN [value]`.
    fn parse_return(&mut self) -> Result<Command> {
        self.advance(); // consume RETURN
        Ok(Command::Return(if self.at_line_end() {
            None
        } else {
            Some(self.parse_value_expr()?)
        }))
    }

    /// Parses `PRINT [value|NEWLINE|IMAGE path|FILE path]`.
    fn parse_print(&mut self) -> Result<Command> {
        self.advance(); // consume PRINT
        Ok(Command::Print(if self.at_line_end() {
            PrintTarget::DataCell
        } else {
            match self.keyword() {
                Some(Keyword::Newline) => {
                    self.advance();
                    PrintTarget::Newline
                }
                Some(Keyword::Image) => {
                    self.advance();
                    PrintTarget::Image(self.parse_value_expr()?)
                }
                Some(Keyword::File) => {
                    self.advance();
                    PrintTarget::File(self.parse_value_expr()?)
                }
                _ => PrintTarget::Value(self.parse_value_expr()?),
            }
        }))
    }

    /// Parses `REMOVE [position|THIS POSITION|TOP|ANY POSITION] [TO [type] variable]`.
    fn parse_remove(&mut self) -> Result<Command> {
        self.advance(); // consume REMOVE
        let from = match self.keyword() {
            Some(Keyword::Top) => {
                self.advance();
                RemoveFrom::Top
            }
            Some(Keyword::Any) => {
                self.advance();
                self.expect_keyword(Keyword::Position)?;
                RemoveFrom::AnyPosition
            }
            Some(Keyword::This) => {
                self.advance();
                self.expect_keyword(Keyword::Position)?;
                RemoveFrom::ThisPosition
            }
            _ if self.at_line_end() || self.at_keyword(Keyword::To) => RemoveFrom::Bottom,
            _ => RemoveFrom::Position(self.parse_value_expr()?),
        };
        let to = self.parse_to_clause()?;
        Ok(Command::Remove { from, to })
    }

    /// Parses `LOAD FILE path [TO [type] variable]`.
    fn parse_load_file(&mut self) -> Result<Command> {
        self.advance();
        self.expect_keyword(Keyword::File)?;
        let path = self.parse_value_expr()?;
        let to = self.parse_to_clause()?;
        Ok(Command::LoadFile { path, to })
    }

    /// Parses `MOVE LAST NODE TO|BY x y`.
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
                "expected TO or BY after MOVE LAST NODE, found {:?}",
                self.peek()
            )));
        };
        let x = self.parse_value_expr()?;
        let y = self.parse_value_expr()?;
        Ok(Command::MoveLastNode { mode, x, y })
    }

    /// Parses `CALL name [WITH ARGUMENTS args] [GIVING variable]`.
    fn parse_call(&mut self) -> Result<Command> {
        self.advance();
        let name = match self.peek() {
            Some(Token::UpperName(name)) => name.clone(),
            Some(Token::Keyword(kw)) => {
                return Err(Error::syntax(format!(
                    "subroutine name cannot be a reserved word: {kw:?}"
                )));
            }
            other => {
                return Err(Error::syntax(format!(
                    "expected a subroutine name, found {other:?}"
                )));
            }
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

        Ok(Command::Call {
            name,
            giving,
            arguments,
        })
    }

    /// Parses `INPUT [type] [TO variable] [WITH PROMPT prompt]`.
    fn parse_input(&mut self) -> Result<Command> {
        self.advance(); // consume INPUT
        let cast = self.eat_data_type();
        let target = self.parse_optional_clause(Keyword::To)?;
        let prompt = if self.eat_keyword(Keyword::With) {
            self.expect_keyword(Keyword::Prompt)?;
            Some(self.parse_value_expr()?)
        } else {
            None
        };
        Ok(Command::Input {
            cast,
            target,
            prompt,
        })
    }
}

/* ----------------------------------------------------------------------------------------------
 * NODE/CHECKPOINT (BODY) LINE PARSING
 * ---------------------------------------------------------------------------------------------- */

enum BodyLine {
    Node(Node),
    Checkpoint(Checkpoint),
}

impl<'a> Parser<'a> {
    /// Parses a coordinate, errors if out of i32 range.
    fn expect_coord(&mut self) -> Result<i32> {
        let n = self.expect_int()?;
        i32::try_from(n).map_err(|_| Error::syntax(format!("coordinate {n} is out of range")))
    }

    /// Parses `(x,y)` into a position.
    fn parse_position(&mut self) -> Result<Position> {
        self.expect_token(Token::LParen)?;
        let x = self.expect_coord()?;
        self.expect_token(Token::Comma)?;
        let y = self.expect_coord()?;
        self.expect_token(Token::RParen)?;
        Ok(Position { x, y })
    }

    /// Parses `(x,y):COMMAND|(CHECKPOINT id)`.
    fn parse_body_line(&mut self) -> Result<BodyLine> {
        let position = self.parse_position()?;
        self.expect_token(Token::Colon)?;

        if self.eat_keyword(Keyword::Checkpoint) {
            let id = self.expect_int()?;
            if id < 0 {
                return Err(Error::InvalidCheckpointId(id));
            }
            Ok(BodyLine::Checkpoint(Checkpoint { position, id }))
        } else {
            let command = self.parse_command()?;
            Ok(BodyLine::Node(Node { position, command }))
        }
    }
}

/* ----------------------------------------------------------------------------------------------
 * SCOPE ASSEMBLY AND SPLITTER
 * ---------------------------------------------------------------------------------------------- */

impl<'a> Parser<'a> {
    /// Parses the node body.
    fn parse_body(&mut self) -> Result<(Vec<Node>, Vec<Checkpoint>)> {
        let mut nodes = Vec::new();
        let mut checkpoints = Vec::new();

        self.skip_newlines();
        while !self.at_end() {
            match self.parse_body_line()? {
                BodyLine::Node(n) => nodes.push(n),
                BodyLine::Checkpoint(c) => checkpoints.push(c),
            }
            self.skip_newlines();
        }

        Ok((nodes, checkpoints))
    }
}

/// Parses one scope: a title line, a metadata blcok and a node body.
fn parse_scope(source: &str, octothorpes: usize) -> Result<(Scope, Option<i64>)> {
    let (title, rest) = split_title(source, octothorpes)?;
    let title = title.to_string();

    let tokens = lexer::tokenize(rest)?;
    let mut parser = Parser::new(&tokens);

    let raw = parser.parse_metadata()?;
    let max_depth = raw.max_depth;
    let metadata = Metadata::from_raw(raw)?;

    let (nodes, checkpoints) = parser.parse_body()?;

    // exactly one START
    match nodes.iter().filter(|n| n.command == Command::Start).count() {
        0 => return Err(Error::MissingStart(title)),
        1 => (),
        _ => return Err(Error::DuplicateStart(title)),
    };

    // bounds checker
    let in_bounds =
        |p: Position| p.x >= 1 && p.x <= metadata.width && p.y >= 1 && p.y <= metadata.height;

    // nodes and checkpoints are bounded
    if let Some(n) = nodes.iter().find(|n| !in_bounds(n.position)) {
        return Err(Error::NodeCenterOutOfBounds {
            x: n.position.x,
            y: n.position.y,
        });
    }
    if let Some(c) = checkpoints.iter().find(|c| !in_bounds(c.position)) {
        return Err(Error::CheckpointCenterOutOfBounds {
            x: c.position.x,
            y: c.position.y,
        });
    }

    Ok((
        Scope {
            title: title.to_string(),
            metadata,
            nodes,
            checkpoints,
        },
        max_depth,
    ))
}

/// Splits `source` into the main program and subroutines.
fn split_scopes(source: &str) -> (&str, Vec<&str>) {
    let mut cuts = Vec::new();
    let mut offset = 0;

    for line in source.split_inclusive('\n') {
        if line.trim_start().starts_with("##") {
            cuts.push(offset);
        }
        offset += line.len();
    }

    let main_end = cuts.first().copied().unwrap_or(source.len());
    let main = &source[..main_end];

    let subs = cuts
        .iter()
        .enumerate()
        .map(|(i, &start)| {
            let end = cuts.get(i + 1).copied().unwrap_or(source.len());
            &source[start..end]
        })
        .collect();

    (main, subs)
}

/* ----------------------------------------------------------------------------------------------
 * MAIN PARSER
 * ---------------------------------------------------------------------------------------------- */

/// Parses a full GridScript source file into a `Program`.
pub fn parse(source: &str) -> Result<Program> {
    let (main_src, sub_srcs) = split_scopes(source);
    let (main, max_depth_raw) = parse_scope(main_src, 1)?;

    let mut subroutines = HashMap::new();
    for src in sub_srcs {
        let (scope, _) = parse_scope(src, 2)?;
        let title = scope.title.clone();
        if subroutines.insert(title.clone(), scope).is_some() {
            return Err(Error::DuplicateSubroutine(title));
        }
    }

    let max_depth = Program::max_depth_from_raw(max_depth_raw)?;

    Ok(Program {
        main,
        subroutines,
        max_depth,
    })
}

#[cfg(test)]
#[path = "unit/parser.rs"]
mod tests;
