use crate::error::{Result};
use crate::program::Program;
use crate::rng::GridScriptRng;

/// Run a fully parsed GridScript program, returning the process exit code
pub fn run(_program: &Program, _rng: &mut GridScriptRng) -> Result<i32> {
    todo!("implement interpreter")
}