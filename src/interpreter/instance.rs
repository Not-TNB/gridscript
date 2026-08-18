use crate::error::{GridScriptError as Error, GridScriptWarning as Warning, Result};
use crate::interpreter::context::RunContext;
use crate::interpreter::node::NodeList;
use crate::interpreter::state::State;
use crate::interpreter::tracer::{Point, ProgramTracer};
use crate::parser::ast::{Command, GoTarget, StoreSource, SwitchCond, ValueExpr};
use crate::program::Scope;
use crate::types::{DataType, Value};

/// An execution frame (i.e. main program or subroutine call)
#[derive(Debug, Clone)]
pub struct Instance<'a> {
    pub tracer: ProgramTracer,
    pub state: State,
    pub nodes: NodeList,
    pub last_node: Option<usize>,
    pub steps_taken: u64,
    pub steps_limit: Option<u64>, // None = unlimited
    pub scope: &'a Scope,         // borrowed for checkpoints/title
}

/// Result of a single execution step wrt. control flow
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
    pub fn new(scope: &'a Scope, steps_limit: Option<u64>, start_pos: (f32, f32)) -> Self {
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
        }
    }
}

/* ----------------------------------------------------------------------------------------------
 * GEOMETRY
 * ---------------------------------------------------------------------------------------------- */

/// True if `point` comes within `radius` of `center` (inclusive)
fn point_in_circle(point: Point, center: Point, radius: f32) -> bool {
    let (dx, dy) = (center.0 - point.0, center.1 - point.1);
    dx * dx + dy * dy <= radius * radius
}

/// True if the segment from `start` to `end` comes within `radius` of `center`
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

fn cast_or_warn(value: Value, target: DataType, ctx: &mut RunContext) -> Value {
    match value.cast_to(target) {
        Some(v) => v,
        None => {
            ctx.warn(Warning::CastFailed { value, target });
            Value::Null
        }
    }
}

impl<'a> Instance<'a> {
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
        let name_str = name_val.to_string();
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
    fn eval_or_current(&self, expr: Option<&ValueExpr>, ctx: &mut RunContext) -> Result<Value> {
        match expr {
            Some(e) => self.eval(e, ctx),
            None => Ok(self.current_value()),
        }
    }

    /// Resolves a value expression used as a write target into a variable name
    fn resolve_target(&self, expr: &ValueExpr, ctx: &mut RunContext) -> Result<String> {
        match expr {
            ValueExpr::Var(name) => Ok(name.clone()),
            ValueExpr::DynamicVar { name, .. } => Ok(self.eval(name, ctx)?.to_string()),
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
            None => {
                let n = match value.cast_to(DataType::Int) {
                    Some(Value::Int(n)) => n,
                    _ => {
                        ctx.warn(Warning::NotAnInt(value));
                        0
                    }
                };
                self.state.set_current_cell(n);
            }
        }
        Ok(())
    }
}

/* ----------------------------------------------------------------------------------------------
 * COMMAND EXECUTION
 * ---------------------------------------------------------------------------------------------- */

impl<'a> Instance<'a> {
    /// Executes one command against this frame, returning where control should go next
    pub fn exec_command(&mut self, command: &Command, ctx: &mut RunContext) -> Result<StepOutcome> {
        match command {
            Command::Start => {} // re-entering is nop
            Command::Home => self.state.home(),
            Command::NextValue => self.state.next_value(),
            Command::NextRow => self.state.next_row(),
            Command::PreviousValue => self.state.previous_value(),
            Command::PreviousRow => self.state.previous_row(),
            Command::Return(expr) => return self.exec_return(expr.as_ref(), ctx),
            Command::Call {
                name,
                arguments,
                giving,
            } => return self.exec_call(name, arguments, giving.as_ref(), ctx),
            Command::Push(expr) => {
                let value = self.eval_or_current(expr.as_ref(), ctx)?;
                self.state.push(value);
            }
            Command::Throw(expr) => {
                let msg = self.eval(expr, ctx)?.to_string();
                return Err(Error::Throw(msg));
            }
            Command::Warn(expr) => {
                let msg = self.eval(expr, ctx)?.to_string();
                ctx.warn(Warning::Custom(msg))
            }
            Command::Shuffle => ctx.rng.shuffle(&mut self.state.buffer),
            Command::Go { target, relative } => {
                return self.exec_go(target, *relative, ctx);
            }
            Command::Switch(cond) => return self.exec_switch(cond, ctx),
            Command::Store { source, target } => self.exec_store(source, target.as_ref(), ctx)?,


            _ => todo!(),
        }
        Ok(StepOutcome::Continue)
    }

    fn exec_return(&self, expr: Option<&ValueExpr>, ctx: &mut RunContext) -> Result<StepOutcome> {
        Ok(StepOutcome::Returned(self.eval_or_current(expr, ctx)?))
    }

    fn exec_call(
        &self,
        name: &str,
        arguments: &[ValueExpr],
        giving: Option<&ValueExpr>,
        ctx: &mut RunContext,
    ) -> Result<StepOutcome> {
        let arguments = arguments
            .iter()
            .map(|expr| self.eval(expr, ctx))
            .collect::<Result<Vec<_>>>()?;
        Ok(StepOutcome::Call {
            name: name.to_string(),
            arguments,
            giving: giving.cloned(),
        })
    }

    /// PRE: relative cannot be true when target is cardinal or random
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
        };
        if should_rotate {
            let cur_dir = i64::from(self.tracer.direction);
            self.tracer.set_direction(cur_dir + 90);
        };
        Ok(StepOutcome::Continue)
    }

    fn exec_store(&mut self, source: &StoreSource, target: Option<&ValueExpr>, ctx: &mut RunContext) -> Result<()> {
        let value = match source {
            StoreSource::Random => Value::Float(ctx.rng.random_unit_float()),
            StoreSource::Value(expr) => self.eval(expr, ctx)?,
        };
        self.store(value, target, None, ctx)
    }
}

/* ----------------------------------------------------------------------------------------------
 * STEP
 * ---------------------------------------------------------------------------------------------- */

impl<'a> Instance<'a> {
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

        for index in triggered {
            self.last_node = Some(index);

            let Some(node) = self.nodes.get(index) else {
                continue;
            };
            let command = node.command.clone();

            // return the remainder if not continue
            match self.exec_command(&command, ctx)? {
                StepOutcome::Continue => {}
                outcome => return Ok(outcome),
            }
        }

        Ok(StepOutcome::Continue)
    }
}

#[cfg(test)]
#[path = "../unit/instance.rs"]
mod tests;
