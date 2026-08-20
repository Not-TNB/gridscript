use std::collections::VecDeque;
use std::io::{self, Write};

/// Sink for program output
pub trait Output {
    fn write(&mut self, bytes: &[u8]) -> io::Result<()>;
    fn flush(&mut self) -> io::Result<()>;

    /// Whether this sink can display images, defaults to false
    fn supports_graphics(&self) -> bool {
        false
    }

    /// Displays an image, only called if `supports_graphics()` is true
    fn write_image(&mut self, _path: &str) -> io::Result<()> {
        unreachable!("write_image called on a sink without graphics support")
    }

    /// Bytes written so far, for sinks that capture rather than stream.
    fn captured(&self) -> Option<&[u8]> {
        None
    }
}

/// Source for program input
pub trait Input {
    /// Reads one line without trailing \n.
    /// `None` on EOF
    fn read_line(&mut self) -> io::Result<Option<String>>;
}

/* ----------------------------------------------------------------------------------------------
 * REAL IO
 * ---------------------------------------------------------------------------------------------- */

pub struct StdOutput;
impl Output for StdOutput {
    fn write(&mut self, bytes: &[u8]) -> io::Result<()> {
        io::stdout().write_all(bytes)
    }
    fn flush(&mut self) -> io::Result<()> {
        io::stdout().flush()
    }
}

pub struct StdInput;
impl Input for StdInput {
    fn read_line(&mut self) -> io::Result<Option<String>> {
        let mut line = String::new();
        match io::stdin().read_line(&mut line)? {
            0 => Ok(None), // EOF
            _ => Ok(Some(line.trim_end_matches(['\r', '\n']).to_string())),
        }
    }
}

/* ----------------------------------------------------------------------------------------------
 * LOGGED IO (E.G. FOR DEBUGGING)
 * ---------------------------------------------------------------------------------------------- */

#[derive(Debug, Default)]
pub struct BufferOut {
    pub bytes: Vec<u8>,
}

impl Output for BufferOut {
    fn write(&mut self, bytes: &[u8]) -> io::Result<()> {
        self.bytes.extend_from_slice(bytes);
        Ok(())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
    fn captured(&self) -> Option<&[u8]> {
        Some(&self.bytes)
    }
}

#[derive(Debug, Default)]
pub struct BufferIn {
    lines: VecDeque<String>,
}

impl BufferIn {
    pub fn new(lines: impl IntoIterator<Item = String>) -> Self {
        Self {
            lines: lines.into_iter().collect(),
        }
    }
}

impl Input for BufferIn {
    fn read_line(&mut self) -> io::Result<Option<String>> {
        Ok(self.lines.pop_front())
    }
}
