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
        Self {
            warnings: Vec::new(),
            rng,
            output,
            input,
        }
    }

    pub fn warn(&mut self, warning: GridScriptWarning) {
        self.warnings.push(warning);
    }

    pub fn print(&mut self, bytes: &[u8]) -> std::io::Result<()> {
        self.output.write(bytes)
    }

    pub fn print_flush(&mut self, bytes: &[u8]) -> std::io::Result<()> {
        self.output.write(bytes)?;
        self.output.flush()
    }

    /// Whether the program output can display images
    pub fn supports_graphics(&self) -> bool {
        self.output.supports_graphics()
    }

    /// Displays an image on the program output.
    /// PRE: `supports_graphics()` is true
    pub fn print_image(&mut self, path: &str) -> std::io::Result<()> {
        self.output.write_image(path)
    }

    pub fn read_line(&mut self) -> std::io::Result<Option<String>> {
        self.input.read_line()
    }

    /// Output captured so far, when the sink supports it. Testing aid.
    pub fn captured_output(&self) -> Option<&[u8]> { self.output.captured() }
}

