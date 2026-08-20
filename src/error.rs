use crate::types::{DataType, Value};
use thiserror::Error;

/// Fatal errors which halt execution
#[derive(Debug, Error, Clone, PartialEq)]
pub enum GridScriptError {
    #[error("syntax error: {0}")]
    SyntaxError(String),

    #[error("node centered at ({x},{y}) is out of bounds")]
    NodeCenterOutOfBounds { x: i32, y: i32 },

    #[error("checkpoint centered at ({x},{y}) is out of bounds")]
    CheckpointCenterOutOfBounds { x: i32, y: i32 },

    #[error("no START node found in {0}")]
    MissingStart(String),

    #[error("more than one START node found in {0}")]
    DuplicateStart(String),

    #[error("no subroutines found with the name {0}")]
    NoSuchSubroutine(String),

    #[error("multiple subroutines found with the name {0}")]
    DuplicateSubroutine(String),

    #[error("checkpoint id must be a non-negative integer, got {0}")]
    InvalidCheckpointId(i64),

    #[error("integer overflow in arithmetic operation")]
    IntegerOverflow,

    #[error("division by zero")]
    DivisionByZero,

    #[error(
        "subroutine '{name}' requested more inputs than its \
    {given} argument(s) provide"
    )]
    TooFewArguments { name: String, given: usize },

    #[error("maximum call depth ({0}) exceeded")]
    MaxDepthExceeded(u32),

    #[error("maximum step count ({0}) exceeded")]
    StepsExceeded(u64),

    #[error("metadata key '{key}' must be >= 1, got {value}")]
    InvalidMetadata { key: String, value: i64 },

    #[error("required metadata key '{0}' is missing")]
    MissingMetadata(String),

    #[error("cannot store to the literal value {0}: a write target must be a variable")]
    InvalidTarget(String),

    #[error("{0}")]
    Throw(String),

    #[error("I/O error: {0}")]
    Io(String),
}

/// Non-fatal and logged, does not stop execution
#[derive(Debug, Error, Clone, PartialEq)]
pub enum GridScriptWarning {
    #[error("cannot cast value {value:?} to {target}, storing NULL")]
    CastFailed { value: Value, target: DataType },

    #[error("file '{0}' not found")]
    FileNotFound(String),

    #[error("GOTO target id is not an integer")]
    NonIntegerCheckpointId,

    #[error("no checkpoint with id {0} exists")]
    NoSuchCheckpoint(i64),

    #[error("buffer position '{0}' is out of range or not an integer")]
    InvalidBufferPosition(String),

    #[error("buffer is empty")]
    EmptyBuffer,

    #[error("output console does not support graphical output")]
    NoGraphicalOutput,

    #[error("cannot cast value {0} to INT for dataspace storage, storing 0")]
    NotAnInt(Value),

    #[error("{0}")]
    Custom(String),
}

pub type Result<T> = std::result::Result<T, GridScriptError>;

impl GridScriptError {
    pub fn syntax(msg: impl Into<String>) -> Self {
        GridScriptError::SyntaxError(msg.into())
    }
}

impl From<std::io::Error> for GridScriptError {
    fn from(e: std::io::Error) -> Self {
        GridScriptError::Io(e.to_string())
    }
}
