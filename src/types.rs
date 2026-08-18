use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Int(i32),
    Float(f32),
    Str(Vec<u8>), // Strings contain 8-bit chars, not UTF-8
    Bool(bool),
    Null,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum DataType {
    Int,
    Float,
    Str,
    Bool,
}

/// Typecasts value to bool
fn cast_to_bool(value: &Value) -> bool {
    match value {
        Value::Int(0) => false,
        Value::Int(_) => true,
        Value::Float(f) => *f != 0.0,
        Value::Str(s) if s.is_empty() || s == b"0" => false,
        Value::Str(_) => true,
        Value::Bool(b) => *b,
        Value::Null => false,
    }
}

/// Attempts to typecast value to float
fn cast_to_float(value: &Value) -> Option<f32> {
    Some(match value {
        Value::Int(i) => *i as f32,
        Value::Float(f) => *f,
        Value::Str(s) => std::str::from_utf8(s).ok()?.trim().parse::<f32>().ok()?,
        Value::Bool(true) => 1.0,
        Value::Null | Value::Bool(false) => 0.0,
    })
}

/// Attempts to typecast value to int
fn cast_to_int(value: &Value) -> Option<i32> {
    cast_to_float(value).map(|f| f as i32)
}

/// Typecasts value to string
fn cast_to_string(value: &Value) -> Vec<u8> {
    match value {
        Value::Int(i) => i.to_string().into_bytes(),
        Value::Float(f) => f.to_string().into_bytes(),
        Value::Str(s) => s.clone(),
        Value::Bool(true) => b"TRUE".to_vec(),
        Value::Bool(false) => b"FALSE".to_vec(),
        Value::Null => Vec::new(),
    }
}

impl Value {
    pub fn cast_to(&self, target: DataType) -> Option<Self> {
        Some(match target {
            DataType::Int => Value::Int(cast_to_int(self)?),
            DataType::Float => Value::Float(cast_to_float(self)?),
            DataType::Str => Value::Str(cast_to_string(self)),
            DataType::Bool => Value::Bool(cast_to_bool(self)),
        })
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Int(i) => write!(f, "{i}"),
            Value::Float(x) => write!(f, "{x}"),
            Value::Str(s) => write!(f, "\"{}\"", String::from_utf8_lossy(s)),
            Value::Bool(b) => write!(f, "{}", if *b { "TRUE" } else { "FALSE" }),
            Value::Null => write!(f, "NULL"),
        }
    }
}

impl fmt::Display for DataType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            DataType::Int => "INT",
            DataType::Float => "FLOAT",
            DataType::Str => "STRING",
            DataType::Bool => "BOOL",
        };
        write!(f, "{s}")
    }
}

#[cfg(test)]
#[path = "unit/types.rs"]
mod tests;
