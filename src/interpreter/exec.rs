use crate::error::{GridScriptError as Error, Result};
use crate::interpreter::context::RunContext;
use crate::interpreter::instance::{Instance, StepOutcome};
use crate::interpreter::tracer::Point;
use crate::parser::ast::Command;
use crate::program::{Program, Scope};
use crate::types::Value;

/* ----------------------------------------------------------------------------------------------
 * SCOPE LOOKUPS
 * ---------------------------------------------------------------------------------------------- */

fn start_position(scope: &Scope) -> Point {
    let pos = scope
        .nodes
        .iter()
        .find(|n| matches!(n.command, Command::Start))
        .expect("No start position found!")
        .position;
    (pos.x as f32, pos.y as f32)
}

fn resolve_steps(scope: &Scope, main: &Scope) -> Option<u64> {
    scope.metadata.steps.or(main.metadata.steps)
}

/* ----------------------------------------------------------------------------------------------
 * FRAME OUTCOME STRUCT AND FRAME EXECUTION
 * ---------------------------------------------------------------------------------------------- */

enum FrameOutcome {
    Returned(Value),
    Exited,
}

fn run_instance(
    instance: &mut Instance,
    program: &Program,
    depth: u32,
    ctx: &mut RunContext,
) -> Result<FrameOutcome> {
    loop {
        let outcome = instance.step(ctx)?;
        match outcome {
            StepOutcome::Continue => continue,
            StepOutcome::Exited => return Ok(FrameOutcome::Exited),
            StepOutcome::Returned(v) => return Ok(FrameOutcome::Returned(v)),
            StepOutcome::Call {
                name,
                arguments,
                giving,
            } => {
                if depth + 1 > program.max_depth {
                    return Err(Error::MaxDepthExceeded(program.max_depth));
                }
                let Some(subroutine) = program.subroutines.get(&name) else {
                    return Err(Error::NoSuchSubroutine(name));
                };
                let mut callee = Instance::new(
                    subroutine,
                    resolve_steps(subroutine, &program.main),
                    start_position(subroutine),
                    Some(arguments),
                );
                let returned = match run_instance(&mut callee, program, depth + 1, ctx)? {
                    FrameOutcome::Returned(v) => Some(v),
                    FrameOutcome::Exited => None,
                };
                instance.store_call_result(returned, &giving, ctx)?;
                continue;
            }
        }
    }
}

/* ----------------------------------------------------------------------------------------------
 * PROGRAM RUNNING
 * ---------------------------------------------------------------------------------------------- */

/// Run a fully parsed GridScript program, returning the process exit code.
pub fn run(program: &Program, ctx: &mut RunContext) -> Result<()> {
    let mut main_instance = Instance::new(
        &program.main,
        resolve_steps(&program.main, &program.main),
        start_position(&program.main),
        None,
    );
    run_instance(&mut main_instance, program, 0, ctx)?;
    Ok(())
}
