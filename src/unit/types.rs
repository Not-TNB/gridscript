//! Unit tests for `types`

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
