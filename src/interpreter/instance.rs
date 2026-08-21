use crate::error::{GridScriptError as Error, GridScriptWarning as Warning, Result};
use crate::interpreter::context::RunContext;
use crate::interpreter::node::NodeList;
use crate::interpreter::state::State;
use crate::interpreter::tracer::{Point, ProgramTracer};
use crate::parser::ast::{
    ArithOp, Command, GoTarget, GotoTarget, MoveMode, Position, PrintTarget, RemoveFrom,
    StoreSource, SwitchCond, ToClause, ValueExpr,
};
use crate::program::Scope;
use crate::types::{DataType, Value};

/* ----------------------------------------------------------------------------------------------
 * INSTANCE & STEP OUTCOME STRUCT
 * ---------------------------------------------------------------------------------------------- */

/// An execution frame (i.e. main program or subroutine call).
#[derive(Debug, Clone)]
pub struct Instance<'a> {
    pub tracer: ProgramTracer,
    pub state: State,
    pub nodes: NodeList,
    pub last_node: Option<usize>,
    pub steps_taken: u64,
    pub steps_limit: Option<u64>, // None = unlimited
    pub scope: &'a Scope,         // borrowed for checkpoints/title
    /// Arguments supplied by the CALL that created this frame.
    /// `Some([])` for a call with no arguments. `None` for the main program reading from StdInput.
    pub arguments: Option<Vec<Value>>,
    /// How many arguments have been consumed by INPUT so far.
    pub arguments_consumed: usize,
}

/// Result of a single execution step wrt. control flow.
#[derive(Debug, Clone, PartialEq)]
pub enum StepOutcome {
    /// Nothing terminal; keep stepping
    Continue,
    /// Tracer exited program space boundary; either halt or implicit RETURN NULL
    Exited,
    /// `RETURN [value]` fired
    Returned(Value),
    /// `CALL` fired
    Call {
        name: String,
        arguments: Vec<Value>,
        giving: Option<ValueExpr>,
    },
}

impl<'a> Instance<'a> {
    pub fn new(
        scope: &'a Scope,
        steps_limit: Option<u64>,
        start_pos: (f32, f32),
        arguments: Option<Vec<Value>>,
    ) -> Self {
        Self {
            tracer: ProgramTracer::new(start_pos),
            state: State::new(
                scope.metadata.data_width as usize,
                scope.metadata.data_height as usize,
            ),
            nodes: NodeList::new(&scope.nodes),
            last_node: None,
            steps_taken: 0,
            steps_limit,
            scope,
            arguments,
            arguments_consumed: 0,
        }
    }
}

/* ----------------------------------------------------------------------------------------------
 * GEOMETRY
 * ---------------------------------------------------------------------------------------------- */

/// True if `point` comes within `radius` of `center` (exclusive).
fn point_in_circle(point: Point, center: Point, radius: f32) -> bool {
    let (dx, dy) = (center.0 - point.0, center.1 - point.1);
    dx * dx + dy * dy < radius * radius
}

/// True if the segment from `start` to `end` comes within `radius` of `center`.
fn segment_intersects_circle(start: Point, end: Point, center: Point, radius: f32) -> bool {
    let (dx, dy) = (end.0 - start.0, end.1 - start.1);
    let (fx, fy) = (center.0 - start.0, center.1 - start.1);

    // project center onto segment
    let len_sq = dx * dx + dy * dy;
    let t = if len_sq == 0.0 {
        0.0
    } else {
        ((fx * dx + fy * dy) / len_sq).clamp(0.0, 1.0)
    };
    let proj = (start.0 + t * dx, start.1 + t * dy);

    point_in_circle(proj, center, radius)
}

/* ----------------------------------------------------------------------------------------------
 * VALUE EVALUATION
 * ---------------------------------------------------------------------------------------------- */

/// Unwraps an optional value, warning and yielding NULL when absent.
fn or_warn(value: Option<Value>, warning: Warning, ctx: &mut RunContext) -> Value {
    match value {
        Some(v) => v,
        None => {
            ctx.warn(warning);
            Value::Null
        }
    }
}

/// Attempts to typecast a value, possibly warning.
fn cast_or_warn(value: Value, target: DataType, ctx: &mut RunContext) -> Value {
    let cast = value.cast_to(target);
    or_warn(cast, Warning::CastFailed { value, target }, ctx)
}

impl Instance<'_> {
    /// Resolves a value expression
    pub fn eval(&self, expr: &ValueExpr, ctx: &mut RunContext) -> Result<Value> {
        match expr {
            ValueExpr::Literal(v) => Ok(v.clone()),
            ValueExpr::Var(name) => Ok(self.state.get_variable(name)),
            ValueExpr::DynamicVar { name, cast } => self.eval_dynamic_var(name, *cast, ctx),
        }
    }

    /// Resolves `THE [VARIABLE|type] NAMED name`
    fn eval_dynamic_var(
        &self,
        name: &ValueExpr,
        cast: Option<DataType>,
        ctx: &mut RunContext,
    ) -> Result<Value> {
        let name_val = self.eval(name, ctx)?;
        let name_str = as_string(name_val, ctx);
        let value = self.state.get_variable(&name_str);
        Ok(match cast {
            None => value,
            Some(target) => cast_or_warn(value, target, ctx),
        })
    }

    /// Value at the data tracer's current cell
    fn current_value(&self) -> Value {
        Value::Int(self.state.current_cell())
    }

    /// Evaluates `expr` if present, else yields the current dataspace value.
    fn eval_or_current(&self, expr: &Option<ValueExpr>, ctx: &mut RunContext) -> Result<Value> {
        match expr {
            Some(e) => self.eval(e, ctx),
            None => Ok(self.current_value()),
        }
    }

    /// Evaluates `expr` if present.
    fn eval_opt(&self, expr: &Option<ValueExpr>, ctx: &mut RunContext) -> Result<Option<Value>> {
        match expr {
            Some(e) => Ok(Some(self.eval(e, ctx)?)),
            None => Ok(None),
        }
    }

    /// Resolves a value expression used as a write target into a variable name
    fn resolve_target(&self, expr: &ValueExpr, ctx: &mut RunContext) -> Result<String> {
        match expr {
            ValueExpr::Var(name) => Ok(name.clone()),
            ValueExpr::DynamicVar { name, .. } => Ok(as_string(self.eval(name, ctx)?, ctx)),
            ValueExpr::Literal(v) => Err(Error::InvalidTarget(v.to_string())),
        }
    }

    /// Stores `value` at `target` if given, else at the current dataspace cell
    fn store(
        &mut self,
        value: Value,
        target: Option<&ValueExpr>,
        cast: Option<DataType>,
        ctx: &mut RunContext,
    ) -> Result<()> {
        let value = match cast {
            Some(t) => cast_or_warn(value, t, ctx),
            None => value,
        };
        match target {
            Some(expr) => {
                let name = self.resolve_target(expr, ctx)?;
                self.state.set_variable(name, value);
            }
            None => self.state.set_current_cell(as_int(value, ctx)),
        }
        Ok(())
    }

    /// Stores `value` according to a `TO [type] variable` clause, applying its cast
    /// if one was given. Falls back to the current dataspace cell when the clause is
    /// absent.
    fn store_to_clause(
        &mut self,
        value: Value,
        to: &Option<ToClause>,
        ctx: &mut RunContext,
    ) -> Result<()> {
        self.store(
            value,
            to.as_ref().map(|t| &t.target),
            to.as_ref().and_then(|t| t.cast),
            ctx,
        )
    }
}

/* ----------------------------------------------------------------------------------------------
 * VALUE CONVERSION
 * ---------------------------------------------------------------------------------------------- */

/// Casts a value to `STRING` and yields its raw bytes. STRING casts never fail
/// per the spec, so an empty result is defensive only.
fn as_bytes(value: Value, ctx: &mut RunContext) -> Vec<u8> {
    match cast_or_warn(value, DataType::Str, ctx) {
        Value::Str(bytes) => bytes,
        _ => Vec::new(),
    }
}

/// Casts a value to `STRING` then as a String.
fn as_string(value: Value, ctx: &mut RunContext) -> String {
    String::from_utf8_lossy(&as_bytes(value, ctx)).into_owned()
}

/// Splits a `STRING` given a separator.
fn split_bytes(subject: &[u8], sep: &[u8]) -> Vec<Vec<u8>> {
    let mut parts = Vec::new();
    let mut rest = subject;
    while let Some(pos) = rest.windows(sep.len()).position(|w| w == sep) {
        parts.push(rest[..pos].to_vec());
        rest = &rest[pos + sep.len()..];
    }
    parts.push(rest.to_vec());
    parts
}

/// Casts a value to INT and yields it, warning and yielding 0 if the cast fails.
fn as_int(value: Value, ctx: &mut RunContext) -> i32 {
    match cast_or_warn(value, DataType::Int, ctx) {
        Value::Int(n) => n,
        _ => 0,
    }
}

/* ----------------------------------------------------------------------------------------------
 * MISCELLANEOUS HELPERS
 * ---------------------------------------------------------------------------------------------- */

impl Instance<'_> {
    /// Removes the buffer value at a signed index, warning and yielding NULL if the
    /// index is negative or OOB.
    fn remove_at_index(&mut self, index: i32, ctx: &mut RunContext) -> Value {
        let Ok(index) = usize::try_from(index) else {
            ctx.warn(Warning::InvalidBufferPosition(index.to_string()));
            return Value::Null;
        };
        or_warn(
            self.state.remove_at(index),
            Warning::InvalidBufferPosition(index.to_string()),
            ctx,
        )
    }

    /// Executes the commands at `indices` in file order, updating `last_node` as
    /// each is entered. Stops early if a command ends or suspends this frame.
    fn execute_nodes(&mut self, indices: Vec<usize>, ctx: &mut RunContext) -> Result<StepOutcome> {
        for index in indices {
            // an earlier command in this batch may have deleted the node
            let Some(node) = self.nodes.get(index) else {
                continue;
            };
            self.last_node = Some(index);
            let command = node.command.clone();

            match self.exec_command(&command, ctx)? {
                StepOutcome::Continue => {}
                outcome => return Ok(outcome),
            }
        }
        Ok(StepOutcome::Continue)
    }

    /// Takes the next unconsumed CALL argument. `None` if this frame has no
    /// argument list (main program) or has exhausted it.
    fn next_argument(&mut self) -> Option<Value> {
        let value = self
            .arguments
            .as_ref()?
            .get(self.arguments_consumed)?
            .clone();
        self.arguments_consumed += 1;
        Some(value)
    }
}

/* ----------------------------------------------------------------------------------------------
 * COMMAND EXECUTION
 * ---------------------------------------------------------------------------------------------- */

impl Instance<'_> {
    /// Executes one command against this frame, returning where control should go next.
    pub fn exec_command(&mut self, command: &Command, ctx: &mut RunContext) -> Result<StepOutcome> {
        match command {
            Command::Start => {} // re-entering is nop
            Command::Home => self.state.home(),
            Command::NextValue => self.state.next_value(),
            Command::NextRow => self.state.next_row(),
            Command::PreviousValue => self.state.previous_value(),
            Command::PreviousRow => self.state.previous_row(),
            Command::Return(expr) => return self.exec_return(expr, ctx),
            Command::Call {
                name,
                arguments,
                giving,
            } => return self.exec_call(name, arguments, giving, ctx),
            Command::Push(expr) => {
                let value = self.eval_or_current(expr, ctx)?;
                self.state.push(value);
            }
            Command::Throw(expr) => {
                let msg = as_string(self.eval(expr, ctx)?, ctx);
                return Err(Error::Throw(msg));
            }
            Command::Warn(expr) => {
                let msg = as_string(self.eval(expr, ctx)?, ctx);
                ctx.warn(Warning::Custom(msg))
            }
            Command::Shuffle => ctx.rng.shuffle(&mut self.state.buffer),
            Command::Go { target, relative } => {
                return self.exec_go(target, *relative, ctx);
            }
            Command::Switch(cond) => return self.exec_switch(cond, ctx),
            Command::Store { source, target } => self.exec_store(source, target, ctx)?,
            Command::Print(target) => self.exec_print(target, ctx)?,
            Command::Peek(to_clause) => self.exec_peek(to_clause, ctx)?,
            Command::Remove { from, to } => self.exec_remove(from, to, ctx)?,
            Command::Split { value, over } => self.exec_split(value, over, ctx)?,
            Command::Arithmetic {
                op,
                target,
                by,
                giving,
            } => self.exec_arithmetic(op, target, by, giving, ctx)?,
            Command::MoveLastNode { mode, x, y } => self.exec_move_last_node(mode, x, y, ctx)?,
            Command::Goto(target) => return self.exec_goto(target, ctx),
            Command::LoadFile { path, to } => self.exec_load_file(path, to, ctx)?,
            Command::Input {
                cast,
                target,
                prompt,
            } => self.exec_input(cast, target, prompt, ctx)?,
        }
        Ok(StepOutcome::Continue)
    }

    fn exec_return(&self, expr: &Option<ValueExpr>, ctx: &mut RunContext) -> Result<StepOutcome> {
        Ok(StepOutcome::Returned(self.eval_or_current(expr, ctx)?))
    }

    fn exec_call(
        &self,
        name: &str,
        arguments: &[ValueExpr],
        giving: &Option<ValueExpr>,
        ctx: &mut RunContext,
    ) -> Result<StepOutcome> {
        let arguments = arguments
            .iter()
            .map(|expr| self.eval(expr, ctx))
            .collect::<Result<Vec<_>>>()?;
        Ok(StepOutcome::Call {
            name: name.to_string(),
            arguments,
            giving: giving.clone(),
        })
    }

    /// PRE: relative cannot be true when target is cardinal or random.
    fn exec_go(
        &mut self,
        target: &GoTarget,
        relative: bool,
        ctx: &mut RunContext,
    ) -> Result<StepOutcome> {
        let cur_dir = i64::from(self.tracer.direction);
        let resolved = match target {
            GoTarget::North => 270,
            GoTarget::South => 90,
            GoTarget::East => 0,
            GoTarget::West => 180,
            GoTarget::Random => i64::from(ctx.rng.random_direction()),
            GoTarget::ThisDirection => i64::from(self.state.current_cell()),
            GoTarget::Value(expr) => {
                let value = self.eval(expr, ctx)?;
                let Some(Value::Int(i)) = value.cast_to(DataType::Int) else {
                    ctx.warn(Warning::NotAnInt(value));
                    return Ok(StepOutcome::Continue); // direction untouched
                };
                i64::from(i)
            }
        };
        self.tracer.set_direction(if relative {
            cur_dir + resolved
        } else {
            resolved
        });
        Ok(StepOutcome::Continue)
    }

    fn exec_switch(&mut self, cond: &SwitchCond, ctx: &mut RunContext) -> Result<StepOutcome> {
        let data = self.state.current_cell();
        let should_rotate = match cond {
            SwitchCond::Random => ctx.rng.coin_flip(),
            SwitchCond::Truthy(expr) => {
                let value = self.eval(expr, ctx)?;
                matches!(value.cast_to(DataType::Bool), Some(Value::Bool(true)))
            }
            SwitchCond::Falsy(expr) => {
                let value = self.eval(expr, ctx)?;
                matches!(value.cast_to(DataType::Bool), Some(Value::Bool(false)))
            }
            SwitchCond::Equals(expr) => {
                let value = self.eval(expr, ctx)?;
                matches!(value.cast_to(DataType::Int), Some(Value::Int(n)) if n == data)
            }
            SwitchCond::NotEquals(expr) => {
                let value = self.eval(expr, ctx)?;
                !matches!(value.cast_to(DataType::Int), Some(Value::Int(n)) if n == data)
            }
            SwitchCond::GreaterThan(expr) => {
                let value = self.eval(expr, ctx)?;
                matches!(value.cast_to(DataType::Int), Some(Value::Int(n)) if data > n)
            }
            SwitchCond::LessThan(expr) => {
                let value = self.eval(expr, ctx)?;
                matches!(value.cast_to(DataType::Int), Some(Value::Int(n)) if data < n)
            }
        };
        if should_rotate {
            let cur_dir = i64::from(self.tracer.direction);
            self.tracer.set_direction(cur_dir + 90);
        }
        Ok(StepOutcome::Continue)
    }

    fn exec_store(
        &mut self,
        source: &StoreSource,
        target: &Option<ValueExpr>,
        ctx: &mut RunContext,
    ) -> Result<()> {
        let value = match source {
            StoreSource::Random => Value::Float(ctx.rng.random_unit_float()),
            StoreSource::Value(expr) => self.eval(expr, ctx)?,
        };
        self.store(value, target.as_ref(), None, ctx)
    }

    fn exec_print(&mut self, target: &PrintTarget, ctx: &mut RunContext) -> Result<()> {
        let to_write = match target {
            PrintTarget::DataCell => as_bytes(self.current_value(), ctx),
            PrintTarget::Value(expr) => as_bytes(self.eval(expr, ctx)?, ctx),
            PrintTarget::Newline => b"\n".to_vec(),
            PrintTarget::File(path) => {
                let path = as_string(self.eval(path, ctx)?, ctx);
                let Ok(contents) = std::fs::read(&path) else {
                    ctx.warn(Warning::FileNotFound(path));
                    return Ok(());
                };
                let mut bytes = vec![b'\n'];
                bytes.extend_from_slice(&contents);
                bytes.push(b'\n');
                bytes
            }
            PrintTarget::Image(path) => {
                let path = as_string(self.eval(path, ctx)?, ctx);
                if !std::path::Path::new(&path).exists() {
                    ctx.warn(Warning::FileNotFound(path));
                    return Ok(());
                }
                if ctx.supports_graphics() {
                    ctx.print(b"\n")?;
                    ctx.print_image(&path)?;
                    ctx.print(b"\n")?;
                    return Ok(());
                }
                ctx.warn(Warning::NoGraphicalOutput);
                path.into_bytes()
            }
        };
        ctx.print(&to_write)?;
        Ok(())
    }

    fn exec_peek(&mut self, to_clause: &Option<ToClause>, ctx: &mut RunContext) -> Result<()> {
        let value = or_warn(self.state.peek(), Warning::EmptyBuffer, ctx);
        self.store_to_clause(value, to_clause, ctx)
    }

    fn exec_remove(
        &mut self,
        from: &RemoveFrom,
        to: &Option<ToClause>,
        ctx: &mut RunContext,
    ) -> Result<()> {
        let removed = match from {
            RemoveFrom::Bottom => or_warn(self.state.remove_bottom(), Warning::EmptyBuffer, ctx),
            RemoveFrom::Top => or_warn(self.state.remove_top(), Warning::EmptyBuffer, ctx),
            RemoveFrom::ThisPosition => {
                let index = self.state.current_cell();
                self.remove_at_index(index, ctx)
            }
            RemoveFrom::AnyPosition => {
                if self.state.buffer.is_empty() {
                    ctx.warn(Warning::EmptyBuffer);
                    Value::Null
                } else {
                    let index = ctx.rng.random_index(self.state.buffer.len());
                    or_warn(self.state.remove_at(index), Warning::EmptyBuffer, ctx)
                }
            }
            RemoveFrom::Position(expr) => {
                let value = self.eval(expr, ctx)?;
                match value.cast_to(DataType::Int) {
                    Some(Value::Int(n)) => self.remove_at_index(n, ctx),
                    _ => {
                        ctx.warn(Warning::InvalidBufferPosition(value.to_string()));
                        Value::Null
                    }
                }
            }
        };
        self.store_to_clause(removed, to, ctx)
    }

    fn exec_split(
        &mut self,
        value: &ValueExpr,
        over: &Option<ValueExpr>,
        ctx: &mut RunContext,
    ) -> Result<()> {
        let subject = as_bytes(self.eval(value, ctx)?, ctx);
        let sep = self.eval_opt(over, ctx)?.map(|v| as_bytes(v, ctx));
        let parts = match sep {
            Some(sep) if !sep.is_empty() => split_bytes(&subject, &sep),
            Some(_) => subject.iter().map(|b| vec![*b]).collect(),
            None => subject
                .split(|b| b.is_ascii_whitespace())
                .filter(|piece| !piece.is_empty())
                .map(<[u8]>::to_vec)
                .collect(),
        };
        parts
            .into_iter()
            .for_each(|part| self.state.push(Value::Str(part)));
        Ok(())
    }

    fn exec_arithmetic(
        &mut self,
        op: &ArithOp,
        target: &Option<ValueExpr>,
        by: &Option<ValueExpr>,
        giving: &Option<ValueExpr>,
        ctx: &mut RunContext,
    ) -> Result<()> {
        let lhs = self.eval_or_current(target, ctx)?;
        let rhs = self.eval_opt(by, ctx)?.unwrap_or(Value::Int(1));

        let (lhs, rhs) = (as_int(lhs, ctx), as_int(rhs, ctx));

        let result = match op {
            ArithOp::Increment => lhs.checked_add(rhs).ok_or(Error::IntegerOverflow)?,
            ArithOp::Decrement => lhs.checked_sub(rhs).ok_or(Error::IntegerOverflow)?,
            ArithOp::Multiply => lhs.checked_mul(rhs).ok_or(Error::IntegerOverflow)?,
            ArithOp::Divide => {
                if rhs == 0 {
                    return Err(Error::DivisionByZero);
                }
                lhs.checked_div(rhs).ok_or(Error::IntegerOverflow)?
            }
        };

        let destination = if giving.is_some() { giving } else { target };
        self.store(Value::Int(result), destination.as_ref(), None, ctx)
    }

    fn exec_move_last_node(
        &mut self,
        mode: &MoveMode,
        x: &ValueExpr,
        y: &ValueExpr,
        ctx: &mut RunContext,
    ) -> Result<()> {
        let x = as_int(self.eval(x, ctx)?, ctx);
        let y = as_int(self.eval(y, ctx)?, ctx);

        let Some(index) = self.last_node else {
            return Ok(());
        };

        let new_pos = match mode {
            MoveMode::To => Position { x, y },
            MoveMode::By => {
                let Some(node) = self.nodes.get(index) else {
                    return Ok(());
                }; // node deleted
                Position {
                    x: node.position.x + x,
                    y: node.position.y + y,
                }
            }
        };

        let (w, h) = (self.scope.metadata.width, self.scope.metadata.height);
        self.nodes.move_node(index, new_pos, |p| {
            p.x >= 1 && p.x <= w && p.y >= 1 && p.y <= h
        });

        Ok(())
    }

    fn exec_goto(&mut self, target: &GotoTarget, ctx: &mut RunContext) -> Result<StepOutcome> {
        // resolve checkpoint id
        let id = match target {
            GotoTarget::ThisCheckpoint => i64::from(self.state.current_cell()),
            GotoTarget::Id(expr) => {
                let value = self.eval(expr, ctx)?;
                let Some(Value::Int(n)) = value.cast_to(DataType::Int) else {
                    ctx.warn(Warning::NonIntegerCheckpointId);
                    return Ok(StepOutcome::Continue);
                };
                i64::from(n)
            }
        };

        // get all checkpoints with this id paired with square distance
        let here = (self.tracer.x, self.tracer.y);
        let candidates: Vec<(f32, Point)> = self
            .scope
            .checkpoints
            .iter()
            .filter(|c| c.id == id)
            .map(|c| {
                let p = (c.position.x as f32, c.position.y as f32);
                let (dx, dy) = (p.0 - here.0, p.1 - here.1);
                (dx * dx + dy * dy, p)
            })
            .collect();

        if candidates.is_empty() {
            ctx.warn(Warning::NoSuchCheckpoint(id));
            return Ok(StepOutcome::Continue);
        }

        // nearest with random tiebreaker
        let nearest = candidates
            .iter()
            .map(|(d, _)| *d)
            .fold(f32::INFINITY, f32::min);
        let tied: Vec<Point> = candidates
            .iter()
            .filter(|(d, _)| *d == nearest)
            .map(|(_, p)| *p)
            .collect();
        let destination = tied[ctx.rng.random_index(tied.len())];

        // tp
        self.tracer.teleport(destination);

        // execute all cmds in landing area
        let radius = self.scope.metadata.radius as f32;
        let landed: Vec<usize> = self
            .nodes
            .iter_live()
            .filter(|(_, n)| {
                let center = (n.position.x as f32, n.position.y as f32);
                point_in_circle(destination, center, radius)
            })
            .map(|(i, _)| i)
            .collect();

        self.execute_nodes(landed, ctx)
    }

    fn exec_load_file(
        &mut self,
        path: &ValueExpr,
        to: &Option<ToClause>,
        ctx: &mut RunContext,
    ) -> Result<()> {
        let path = as_string(self.eval(path, ctx)?, ctx);
        let contents = match std::fs::read(&path) {
            Ok(bytes) => Value::Str(bytes),
            Err(_) => {
                ctx.warn(Warning::FileNotFound(path));
                Value::Null
            }
        };
        self.store_to_clause(contents, to, ctx)
    }

    fn exec_input(
        &mut self,
        cast: &Option<DataType>,
        target: &Option<ValueExpr>,
        prompt: &Option<ValueExpr>,
        ctx: &mut RunContext,
    ) -> Result<()> {
        let value = if self.arguments.is_some() {
            // subroutine frame: consume next CALL argument
            let Some(value) = self.next_argument() else {
                return Err(Error::TooFewArguments {
                    name: self.scope.title.clone(),
                    given: self.arguments_consumed,
                });
            };
            value
        } else {
            // main program: prompt if asked then read from StdInput
            if let Some(expr) = prompt {
                let message = as_bytes(self.eval(expr, ctx)?, ctx);
                ctx.print_flush(&message)?;
            }
            match ctx.read_line()? {
                Some(line) => Value::Str(line.into_bytes()),
                None => Value::Null,
            }
        };
        self.store(value, target.as_ref(), *cast, ctx)
    }
}

/* ----------------------------------------------------------------------------------------------
 * EXPOSED METHODS
 * ---------------------------------------------------------------------------------------------- */

impl Instance<'_> {
    pub fn step(&mut self, ctx: &mut RunContext) -> Result<StepOutcome> {
        // check if max steps exceeded
        if let Some(lim) = self.steps_limit
            && self.steps_taken >= lim
        {
            return Err(Error::StepsExceeded(lim));
        }
        self.steps_taken += 1;

        let (start, end) = self.tracer.advance();

        // boundary check
        let (w, h) = (
            self.scope.metadata.width as f32,
            self.scope.metadata.height as f32,
        );
        if end.0 < 1.0 || end.0 > w || end.1 < 1.0 || end.1 > h {
            return Ok(StepOutcome::Exited);
        }

        // collect indices of nodes to trigger
        let radius = self.scope.metadata.radius as f32;
        let triggered: Vec<usize> = self
            .nodes
            .iter_live()
            .filter(|(_, n)| {
                let center = (n.position.x as f32, n.position.y as f32);
                segment_intersects_circle(start, end, center, radius)
                    && !point_in_circle(start, center, radius) // wasn't already triggered
            })
            .map(|(i, _)| i)
            .collect();

        self.execute_nodes(triggered, ctx)
    }

    pub fn store_call_result(
        &mut self,
        returned: Option<Value>,
        giving: &Option<ValueExpr>,
        ctx: &mut RunContext,
    ) -> Result<()> {
        match (returned, giving) {
            (None, None) => Ok(()),
            (v, g) => self.store(v.unwrap_or(Value::Null), g.as_ref(), None, ctx),
        }
    }
}

#[cfg(test)]
#[path = "../unit/instance.rs"]
mod tests;
