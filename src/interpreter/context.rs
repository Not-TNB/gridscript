use crate::error::GridScriptWarning;
use crate::interpreter::io::{Input, Output};
use crate::rng::GridScriptRng;

/// Program-wide state shared across all execution frames:
/// warning log, RNG stream, I/O pair
pub struct RunContext {
    pub warnings: Vec<GridScriptWarning>,
    pub rng: GridScriptRng,
    pub output: Box<dyn Output>,
    pub input: Box<dyn Input>,
}

impl RunContext {
    pub fn new(rng: GridScriptRng, output: Box<dyn Output>, input: Box<dyn Input>) -> Self {
        Self { warnings: Vec::new(), rng, output, input }
    }

    pub fn warn(&mut self, warning: GridScriptWarning) {
        self.warnings.push(warning);
    }

    /// Writes bytes to program output
    pub fn print(&mut self, bytes: &[u8]) -> std::io::Result<()> {
        self.output.write(bytes)
    }

    /// Writes bytes followed by a flush
    pub fn print_flush(&mut self, bytes: &[u8]) -> std::io::Result<()> {
        self.output.write(bytes)?;
        self.output.flush()
    }
}
