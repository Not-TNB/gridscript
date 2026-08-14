pub mod error;
pub mod interpreter;
pub mod parser;
pub mod program;
pub mod rng;
pub mod types;

use crate::error::Result;
use rng::GridScriptRng;

pub fn run(source: &str, seed_override: Option<u64>) -> Result<i32> {
    let program = parser::parse(source)?;
    let seed = seed_override.or(program.main.metadata.seed);
    let mut rng = match seed {
        Some(s) => GridScriptRng::from_seed(s),
        None => GridScriptRng::from_entropy(),
    };
    interpreter::exec::run(&program, &mut rng)
}
