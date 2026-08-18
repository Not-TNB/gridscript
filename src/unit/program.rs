//! Unit tests for `program`

use super::*;

#[test]
fn defaults_to_1000_when_absent() {
    assert_eq!(Program::max_depth_from_raw(None).unwrap(), 1000);
}

#[test]
fn accepts_valid_values() {
    assert_eq!(Program::max_depth_from_raw(Some(1)).unwrap(), 1);
    assert_eq!(Program::max_depth_from_raw(Some(50)).unwrap(), 50);
    assert_eq!(
        Program::max_depth_from_raw(Some(1_000_000)).unwrap(),
        1_000_000
    );
}

#[test]
fn rejects_zero() {
    assert!(Program::max_depth_from_raw(Some(0)).is_err());
}

#[test]
fn rejects_negative() {
    assert!(Program::max_depth_from_raw(Some(-5)).is_err());
}

#[test]
fn rejects_values_beyond_u32_range() {
    // i64 value that doesn't fit in u32
    assert!(Program::max_depth_from_raw(Some(i64::from(u32::MAX) + 1)).is_err());
}

#[test]
fn error_reports_the_offending_value() {
    match Program::max_depth_from_raw(Some(-5)) {
        Err(GridScriptError::InvalidMetadata { key, value }) => {
            assert_eq!(key, "maxdepth");
            assert_eq!(value, -5);
        }
        other => panic!("expected InvalidMetadata, got {other:?}"),
    }
}
