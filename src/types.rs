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
    Int, Float, Str, Bool,
}

/// Typecasts value to bool
fn cast_to_bool(value: &Value) -> bool {
    match value {
        Value::Int(0) => false, Value::Int(_) => true,
        Value::Float(f) => *f != 0.0,
        Value::Str(s) if s.is_empty() || s == b"0" => false, Value::Str(_) => true,
        Value::Bool(b) => *b,
        Value::Null => false,
    }
}

/// Attempts to typecast value to float
fn cast_to_float(value: &Value) -> Option<f32> {
    Some(match value {
        Value::Int(i) => *i as f32,
        Value::Float(f) => *f,
        Value::Str(s) => {
            std::str::from_utf8(s)
                .ok()?
                .trim()
                .parse::<f32>()
                .ok()?
        },
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
        Value::Bool(true) => b"TRUE".to_vec(), Value::Bool(false) => b"FALSE".to_vec(),
        Value::Null => Vec::new(),
    }
}

impl Value {
    pub fn cast_to(&self, target: DataType) -> Option<Self> {
        Some(match target {
            DataType::Int   => Value::Int(cast_to_int(self)?),
            DataType::Float => Value::Float(cast_to_float(self)?),
            DataType::Str   => Value::Str(cast_to_string(self)),
            DataType::Bool  => Value::Bool(cast_to_bool(self)),
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
mod tests {
    use super::*;

    #[test]
    fn bool_casts() {
        assert_eq!(cast_to_bool(&Value::Int(0)), false);
        assert_eq!(cast_to_bool(&Value::Int(1)), true);
        assert_eq!(cast_to_bool(&Value::Float(0.0)), false);
        assert_eq!(cast_to_bool(&Value::Float(0.1)), true);
        assert_eq!(cast_to_bool(&Value::Str(Vec::new())), false);
        assert_eq!(cast_to_bool(&Value::Str(b"0".to_vec())), false);
        assert_eq!(cast_to_bool(&Value::Str(b"FALSE".to_vec())), true);
        assert_eq!(cast_to_bool(&Value::Bool(true)), true);
        assert_eq!(cast_to_bool(&Value::Bool(false)), false);
        assert_eq!(cast_to_bool(&Value::Null), false);
    }

    #[test]
    fn float_casts() {
        assert_eq!(cast_to_float(&Value::Int(7)), Some(7.0));
        assert_eq!(cast_to_float(&Value::Bool(true)), Some(1.0));
        assert_eq!(cast_to_float(&Value::Null), Some(0.0));
        assert_eq!(cast_to_float(&Value::Str(b"  3.5  ".to_vec())), Some(3.5));
        assert_eq!(cast_to_float(&Value::Str(b"1e3".to_vec())), Some(1000.0));
        assert_eq!(cast_to_float(&Value::Str(b"hello".to_vec())), None);
        assert_eq!(cast_to_float(&Value::Str(vec![0xff, 0xfe])), None);
    }

    #[test]
    fn int_casts_truncate_toward_zero() {
        assert_eq!(cast_to_int(&Value::Float(3.9)), Some(3));
        assert_eq!(cast_to_int(&Value::Float(-3.9)), Some(-3));
        assert_eq!(cast_to_int(&Value::Str(b"7.8".to_vec())), Some(7));
    }

    #[test]
    fn string_casts() {
        assert_eq!(cast_to_string(&Value::Null), Vec::<u8>::new());
        assert_eq!(cast_to_string(&Value::Bool(true)), b"TRUE".to_vec());
        assert_eq!(cast_to_string(&Value::Bool(false)), b"FALSE".to_vec());
        assert_eq!(cast_to_string(&Value::Int(-17)), b"-17".to_vec());
        assert_eq!(cast_to_string(&Value::Str(b"x".to_vec())), b"x".to_vec());
    }

    #[test]
    fn bool_false_does_not_round_trip_through_string() {
        let s = Value::Bool(false).cast_to(DataType::Str).unwrap();
        assert_eq!(s.cast_to(DataType::Bool).unwrap(), Value::Bool(true));
    }

    #[test]
    fn cast_to_wrapper() {
        let v = Value::Str(b"not a number".to_vec());
        assert_eq!(v.cast_to(DataType::Int), None);
        assert!(v.cast_to(DataType::Str).is_some());
    }

    #[test]
    fn value_display_quotes_strings_and_names_null() {
        assert_eq!(Value::Str(b"hi".to_vec()).to_string(), "\"hi\"");
        assert_eq!(Value::Null.to_string(), "NULL");
        assert_eq!(Value::Bool(false).to_string(), "FALSE");
    }

    #[test]
    fn datatype_display_matches_spec_keywords() {
        assert_eq!(DataType::Str.to_string(), "STRING");
        assert_eq!(DataType::Bool.to_string(), "BOOL");
    }
}
