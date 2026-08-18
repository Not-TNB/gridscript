use std::io::{self, BufRead, Write};
use std::collections::VecDeque;

/// Sink for program output
pub trait Output {
    fn write(&mut self, bytes: &[u8]) -> io::Result<()>;
    fn flush(&mut self) -> io::Result<()>;
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

pub struct StdOut;
impl Output for StdOut {
    fn write(&mut self, bytes: &[u8]) -> io::Result<()> {
        io::stdout().write_all(bytes)
    }
    fn flush(&mut self) -> io::Result<()> {
        io::stdout().flush()
    }
}

pub struct StdIn;
impl Input for StdIn {
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
    fn flush(&mut self) -> io::Result<()> { Ok(()) }
}

#[derive(Debug, Default)]
pub struct BufferIn {
    lines: VecDeque<String>,
}

impl Input for BufferIn {
    fn read_line(&mut self) -> io::Result<Option<String>> {
        Ok(self.lines.pop_front())
    }
}
