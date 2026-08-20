//! Unit tests for `interpreter::instance`

use crate::interpreter::io::{BufferIn, BufferOut};
use crate::parser::ast::{DebugMode, Node};
use crate::program::Metadata;
use crate::rng::GridScriptRng;
use super::*;

fn int(n: i32) -> ValueExpr {
    ValueExpr::Literal(Value::Int(n))
}
fn var(s: &str) -> ValueExpr {
    ValueExpr::Var(s.into())
}
fn str_lit(s: &str) -> ValueExpr {
    ValueExpr::Literal(Value::Str(s.as_bytes().to_vec()))
}


/* --- geometry --- */

#[test]
fn point_containment() {
    let center = (5.0, 5.0);

    assert!(point_in_circle((5.0, 5.0), center, 1.0)); // dead center
    assert!(point_in_circle((5.5, 5.0), center, 1.0)); // inside
    assert!(point_in_circle((6.0, 5.0), center, 1.0)); // exactly on the edge
    assert!(!point_in_circle((6.1, 5.0), center, 1.0)); // just outside
    assert!(!point_in_circle((9.0, 9.0), center, 1.0)); // far away
}

#[test]
fn segment_through_circle() {
    let center = (5.0, 5.0);
    // horizontal segment passing straight through the center
    assert!(segment_intersects_circle(
        (3.0, 5.0),
        (7.0, 5.0),
        center,
        1.0
    ));
}

#[test]
fn segment_missing_circle() {
    let center = (5.0, 5.0);
    // parallel but too far north
    assert!(!segment_intersects_circle(
        (3.0, 2.0),
        (7.0, 2.0),
        center,
        1.0
    ));
}

#[test]
fn segment_grazing_edge() {
    let center = (5.0, 5.0);
    // passes exactly `radius` away — inclusive, so this counts
    assert!(segment_intersects_circle(
        (3.0, 4.0),
        (7.0, 4.0),
        center,
        1.0
    ));
}

#[test]
fn segment_stops_short_of_circle() {
    // The clamp's reason for existing: the circle lies on the infinite line
    // through this segment, but beyond its end. An unclamped projection would
    // wrongly report a hit here.
    let center = (10.0, 5.0);
    assert!(!segment_intersects_circle(
        (1.0, 5.0),
        (3.0, 5.0),
        center,
        1.0
    ));
}

#[test]
fn segment_starting_inside() {
    let center = (5.0, 5.0);
    // begins within the circle and exits it
    assert!(segment_intersects_circle(
        (5.0, 5.0),
        (8.0, 5.0),
        center,
        1.0
    ));
}

#[test]
fn tiny_circle_is_not_skipped() {
    // The spec's stated motivation for a segment test over endpoint tests: a
    // node smaller than one step's travel must still trigger when crossed.
    let center = (5.0, 5.0);
    let radius = 0.1;
    // neither endpoint is inside, but the segment passes through
    assert!(!point_in_circle((4.0, 5.0), center, radius));
    assert!(!point_in_circle((6.0, 5.0), center, radius));
    assert!(segment_intersects_circle(
        (4.0, 5.0),
        (6.0, 5.0),
        center,
        radius
    ));
}

#[test]
fn degenerate_zero_length_segment() {
    let center = (5.0, 5.0);
    // start == end: falls back to a point test at that position
    assert!(segment_intersects_circle(
        (5.0, 5.0),
        (5.0, 5.0),
        center,
        1.0
    ));
    assert!(!segment_intersects_circle(
        (9.0, 9.0),
        (9.0, 9.0),
        center,
        1.0
    ));
}

/// A minimal scope with just a START node, for tests that only exercise
/// command execution rather than movement.
fn test_scope() -> Scope {
    Scope {
        title: "TEST".into(),
        metadata: Metadata {
            width: 10, height: 10,
            data_width: 4, data_height: 4,
            radius: 1, steps: None,
            debug: DebugMode::False, seed: None,
        },
        nodes: vec![Node { position: Position { x: 1, y: 1 }, command: Command::Start }],
        checkpoints: vec![],
    }
}

/// A context with captured output and scripted input.
fn test_ctx(input: Vec<String>) -> RunContext {
    RunContext::new(
        GridScriptRng::from_seed(0),
        Box::new(BufferOut::default()),
        Box::new(BufferIn::new(input)),
    )
}


#[test]
fn store_writes_to_a_variable() {
    let scope = test_scope();
    let mut inst = Instance::new(&scope, None, (1.0, 1.0), None);
    let mut ctx = test_ctx(vec![]);

    inst.exec_command(
        &Command::Store {
            source: StoreSource::Value(ValueExpr::Literal(Value::Int(42))),
            target: Some(ValueExpr::Var("x".into())),
        },
        &mut ctx,
    ).unwrap();

    assert_eq!(inst.state.get_variable("x"), Value::Int(42));
}

/* --- PRINT --- */

#[test]
fn print_writes_values_and_newlines() {
    let scope = test_scope();
    let mut inst = Instance::new(&scope, None, (1.0, 1.0), None);
    let mut ctx = test_ctx(vec![]);

    inst.exec_command(
        &Command::Print(PrintTarget::Value(ValueExpr::Literal(Value::Str(
            b"hi".to_vec(),
        )))),
        &mut ctx,
    )
        .unwrap();
    inst.exec_command(&Command::Print(PrintTarget::Newline), &mut ctx)
        .unwrap();

    assert_eq!(ctx.captured_output().unwrap(), b"hi\n");
}

#[test]
fn print_defaults_to_the_dataspace_cell() {
    let scope = test_scope();
    let mut inst = Instance::new(&scope, None, (1.0, 1.0), None);
    let mut ctx = test_ctx(vec![]);

    inst.state.set_current_cell(42);
    inst.exec_command(&Command::Print(PrintTarget::DataCell), &mut ctx)
        .unwrap();

    assert_eq!(ctx.captured_output().unwrap(), b"42");
}

#[test]
fn print_missing_file_warns_and_writes_nothing() {
    let scope = test_scope();
    let mut inst = Instance::new(&scope, None, (1.0, 1.0), None);
    let mut ctx = test_ctx(vec![]);

    inst.exec_command(
        &Command::Print(PrintTarget::File(ValueExpr::Literal(Value::Str(
            b"definitely-not-a-real-file.txt".to_vec(),
        )))),
        &mut ctx,
    )
        .unwrap();

    assert!(ctx.captured_output().unwrap().is_empty());
    assert!(matches!(
        ctx.warnings.as_slice(),
        [Warning::FileNotFound(_)]
    ));
}

#[test]
fn print_image_warns_and_falls_back_to_the_filename() {
    let scope = test_scope();
    let mut inst = Instance::new(&scope, None, (1.0, 1.0), None);
    let mut ctx = test_ctx(vec![]);

    // an existing file — Cargo.toml is always present at the crate root
    inst.exec_command(
        &Command::Print(PrintTarget::Image(ValueExpr::Literal(Value::Str(
            b"Cargo.toml".to_vec(),
        )))),
        &mut ctx,
    )
        .unwrap();

    assert_eq!(ctx.captured_output().unwrap(), b"Cargo.toml");
    assert!(matches!(
        ctx.warnings.as_slice(),
        [Warning::NoGraphicalOutput]
    ));
}

/* --- arithmetic --- */

fn arith(op: ArithOp, target: Option<ValueExpr>, by: Option<ValueExpr>, giving: Option<ValueExpr>) -> Command {
    Command::Arithmetic { op, target, by, giving }
}

#[test]
fn increment_defaults_to_one_and_to_the_dataspace() {
    let scope = test_scope();
    let mut inst = Instance::new(&scope, None, (1.0, 1.0), None);
    let mut ctx = test_ctx(vec![]);

    inst.state.set_current_cell(5);
    inst.exec_command(&arith(ArithOp::Increment, None, None, None), &mut ctx).unwrap();

    assert_eq!(inst.state.current_cell(), 6);
}

#[test]
fn arithmetic_operates_on_a_named_variable() {
    let scope = test_scope();
    let mut inst = Instance::new(&scope, None, (1.0, 1.0), None);
    let mut ctx = test_ctx(vec![]);

    inst.state.set_variable("x".into(), Value::Int(10));
    inst.exec_command(&arith(ArithOp::Decrement, Some(var("x")), Some(int(3)), None), &mut ctx).unwrap();

    assert_eq!(inst.state.get_variable("x"), Value::Int(7));
}

#[test]
fn giving_redirects_without_mutating_the_original() {
    let scope = test_scope();
    let mut inst = Instance::new(&scope, None, (1.0, 1.0), None);
    let mut ctx = test_ctx(vec![]);

    inst.state.set_variable("x".into(), Value::Int(4));
    inst.exec_command(
        &arith(ArithOp::Multiply, Some(var("x")), Some(int(3)), Some(var("y"))),
        &mut ctx,
    ).unwrap();

    assert_eq!(inst.state.get_variable("x"), Value::Int(4));  // untouched
    assert_eq!(inst.state.get_variable("y"), Value::Int(12));
}

#[test]
fn overflow_throws() {
    let scope = test_scope();
    let mut inst = Instance::new(&scope, None, (1.0, 1.0), None);
    let mut ctx = test_ctx(vec![]);

    inst.state.set_variable("x".into(), Value::Int(i32::MAX));
    let result = inst.exec_command(&arith(ArithOp::Increment, Some(var("x")), Some(int(1)), None), &mut ctx);

    assert!(matches!(result, Err(Error::IntegerOverflow)));
}

#[test]
fn division_by_zero_throws() {
    let scope = test_scope();
    let mut inst = Instance::new(&scope, None, (1.0, 1.0), None);
    let mut ctx = test_ctx(vec![]);

    inst.state.set_variable("x".into(), Value::Int(10));
    let result = inst.exec_command(&arith(ArithOp::Divide, Some(var("x")), Some(int(0)), None), &mut ctx);

    assert!(matches!(result, Err(Error::DivisionByZero)));
}

#[test]
fn division_truncates_toward_zero() {
    let scope = test_scope();
    let mut inst = Instance::new(&scope, None, (1.0, 1.0), None);
    let mut ctx = test_ctx(vec![]);

    inst.state.set_variable("x".into(), Value::Int(7));
    inst.exec_command(&arith(ArithOp::Divide, Some(var("x")), Some(int(2)), None), &mut ctx).unwrap();

    assert_eq!(inst.state.get_variable("x"), Value::Int(3));
}

/* --- buffer --- */

#[test]
fn push_and_peek_are_fifo() {
    let scope = test_scope();
    let mut inst = Instance::new(&scope, None, (1.0, 1.0), None);
    let mut ctx = test_ctx(vec![]);

    inst.exec_command(&Command::Push(Some(int(1))), &mut ctx).unwrap();
    inst.exec_command(&Command::Push(Some(int(2))), &mut ctx).unwrap();

    // PEEK reads the bottom, which is the first value pushed
    inst.exec_command(&Command::Peek(Some(ToClause { cast: None, target: var("x") })), &mut ctx).unwrap();
    assert_eq!(inst.state.get_variable("x"), Value::Int(1));
    assert_eq!(inst.state.buffer.len(), 2); // PEEK does not modify the buffer
}

#[test]
fn remove_takes_from_the_bottom_by_default() {
    let scope = test_scope();
    let mut inst = Instance::new(&scope, None, (1.0, 1.0), None);
    let mut ctx = test_ctx(vec![]);

    inst.state.push(Value::Int(1));
    inst.state.push(Value::Int(2));

    inst.exec_command(
        &Command::Remove { from: RemoveFrom::Bottom, to: Some(ToClause { cast: None, target: var("x") }) },
        &mut ctx,
    ).unwrap();

    assert_eq!(inst.state.get_variable("x"), Value::Int(1));
    assert_eq!(inst.state.buffer, vec![Value::Int(2)]);
}

#[test]
fn remove_top_takes_the_last_pushed() {
    let scope = test_scope();
    let mut inst = Instance::new(&scope, None, (1.0, 1.0), None);
    let mut ctx = test_ctx(vec![]);

    inst.state.push(Value::Int(1));
    inst.state.push(Value::Int(2));

    inst.exec_command(
        &Command::Remove { from: RemoveFrom::Top, to: Some(ToClause { cast: None, target: var("x") }) },
        &mut ctx,
    ).unwrap();

    assert_eq!(inst.state.get_variable("x"), Value::Int(2));
}

#[test]
fn remove_at_position_indexes_from_the_bottom() {
    let scope = test_scope();
    let mut inst = Instance::new(&scope, None, (1.0, 1.0), None);
    let mut ctx = test_ctx(vec![]);

    inst.state.push(Value::Int(10));
    inst.state.push(Value::Int(20));
    inst.state.push(Value::Int(30));

    // position 0 is the bottom, per spec
    inst.exec_command(
        &Command::Remove { from: RemoveFrom::Position(int(0)), to: Some(ToClause { cast: None, target: var("x") }) },
        &mut ctx,
    ).unwrap();

    assert_eq!(inst.state.get_variable("x"), Value::Int(10));
}

#[test]
fn remove_out_of_range_warns_and_stores_null() {
    let scope = test_scope();
    let mut inst = Instance::new(&scope, None, (1.0, 1.0), None);
    let mut ctx = test_ctx(vec![]);

    inst.state.push(Value::Int(1));

    inst.exec_command(
        &Command::Remove { from: RemoveFrom::Position(int(99)), to: Some(ToClause { cast: None, target: var("x") }) },
        &mut ctx,
    ).unwrap();

    assert_eq!(inst.state.get_variable("x"), Value::Null);
    assert!(matches!(ctx.warnings.as_slice(), [Warning::InvalidBufferPosition(_)]));
}

#[test]
fn peek_on_empty_buffer_warns() {
    let scope = test_scope();
    let mut inst = Instance::new(&scope, None, (1.0, 1.0), None);
    let mut ctx = test_ctx(vec![]);

    inst.exec_command(&Command::Peek(Some(ToClause { cast: None, target: var("x") })), &mut ctx).unwrap();

    assert_eq!(inst.state.get_variable("x"), Value::Null);
    assert!(matches!(ctx.warnings.as_slice(), [Warning::EmptyBuffer]));
}

#[test]
fn remove_to_dataspace_casts_to_int() {
    let scope = test_scope();
    let mut inst = Instance::new(&scope, None, (1.0, 1.0), None);
    let mut ctx = test_ctx(vec![]);

    inst.state.push(Value::Str(b"42".to_vec()));
    inst.exec_command(&Command::Remove { from: RemoveFrom::Bottom, to: None }, &mut ctx).unwrap();

    assert_eq!(inst.state.current_cell(), 42);
}

/* --- SWITCH --- */

#[test]
fn switch_rotates_ninety_degrees_clockwise() {
    let scope = test_scope();
    let mut inst = Instance::new(&scope, None, (1.0, 1.0), None);
    let mut ctx = test_ctx(vec![]);

    assert_eq!(inst.tracer.direction, 0); // east
    inst.exec_command(&Command::Switch(SwitchCond::Truthy(int(1))), &mut ctx).unwrap();
    assert_eq!(inst.tracer.direction, 90); // south
}

#[test]
fn switch_leaves_direction_alone_when_false() {
    let scope = test_scope();
    let mut inst = Instance::new(&scope, None, (1.0, 1.0), None);
    let mut ctx = test_ctx(vec![]);

    inst.exec_command(&Command::Switch(SwitchCond::Truthy(int(0))), &mut ctx).unwrap();
    assert_eq!(inst.tracer.direction, 0);
}

#[test]
fn switch_falsy_inverts_the_test_not_the_rotation() {
    let scope = test_scope();
    let mut inst = Instance::new(&scope, None, (1.0, 1.0), None);
    let mut ctx = test_ctx(vec![]);

    // !0 is true, so this rotates — clockwise, same as any other switch
    inst.exec_command(&Command::Switch(SwitchCond::Falsy(int(0))), &mut ctx).unwrap();
    assert_eq!(inst.tracer.direction, 90);
}

#[test]
fn switch_compares_against_the_dataspace_cell() {
    let scope = test_scope();
    let mut inst = Instance::new(&scope, None, (1.0, 1.0), None);
    let mut ctx = test_ctx(vec![]);

    inst.state.set_current_cell(7);

    inst.exec_command(&Command::Switch(SwitchCond::Equals(int(7))), &mut ctx).unwrap();
    assert_eq!(inst.tracer.direction, 90);

    inst.exec_command(&Command::Switch(SwitchCond::Equals(int(3))), &mut ctx).unwrap();
    assert_eq!(inst.tracer.direction, 90); // unchanged
}

#[test]
fn switch_not_equals_is_the_complement() {
    let scope = test_scope();
    let mut inst = Instance::new(&scope, None, (1.0, 1.0), None);
    let mut ctx = test_ctx(vec![]);

    inst.state.set_current_cell(7);
    inst.exec_command(&Command::Switch(SwitchCond::NotEquals(int(3))), &mut ctx).unwrap();
    assert_eq!(inst.tracer.direction, 90);
}

#[test]
fn switch_wraps_past_360() {
    let scope = test_scope();
    let mut inst = Instance::new(&scope, None, (1.0, 1.0), None);
    let mut ctx = test_ctx(vec![]);

    inst.tracer.set_direction(300);
    inst.exec_command(&Command::Switch(SwitchCond::Truthy(int(1))), &mut ctx).unwrap();
    assert_eq!(inst.tracer.direction, 30); // 300 + 90 = 390 -> 30
}

/* --- GO --- */

#[test]
fn go_maps_cardinals_to_the_spec_degrees() {
    let scope = test_scope();
    let mut ctx = test_ctx(vec![]);

    let cases = [
        (GoTarget::East, 0u16),
        (GoTarget::South, 90),
        (GoTarget::West, 180),
        (GoTarget::North, 270),
    ];
    for (target, expected) in cases {
        let mut inst = Instance::new(&scope, None, (1.0, 1.0), None);
        inst.exec_command(&Command::Go { target, relative: false }, &mut ctx).unwrap();
        assert_eq!(inst.tracer.direction, expected);
    }
}

#[test]
fn go_relative_adds_to_the_current_direction() {
    let scope = test_scope();
    let mut inst = Instance::new(&scope, None, (1.0, 1.0), None);
    let mut ctx = test_ctx(vec![]);

    inst.tracer.set_direction(100);
    inst.exec_command(&Command::Go { target: GoTarget::Value(int(45)), relative: true }, &mut ctx).unwrap();
    assert_eq!(inst.tracer.direction, 145);
}

#[test]
fn go_absolute_replaces_the_current_direction() {
    let scope = test_scope();
    let mut inst = Instance::new(&scope, None, (1.0, 1.0), None);
    let mut ctx = test_ctx(vec![]);

    inst.tracer.set_direction(100);
    inst.exec_command(&Command::Go { target: GoTarget::Value(int(45)), relative: false }, &mut ctx).unwrap();
    assert_eq!(inst.tracer.direction, 45);
}

#[test]
fn go_this_direction_reads_the_dataspace() {
    let scope = test_scope();
    let mut inst = Instance::new(&scope, None, (1.0, 1.0), None);
    let mut ctx = test_ctx(vec![]);

    inst.state.set_current_cell(270);
    inst.exec_command(&Command::Go { target: GoTarget::ThisDirection, relative: false }, &mut ctx).unwrap();
    assert_eq!(inst.tracer.direction, 270);
}

#[test]
fn go_with_an_uncastable_value_leaves_direction_alone() {
    let scope = test_scope();
    let mut inst = Instance::new(&scope, None, (1.0, 1.0), None);
    let mut ctx = test_ctx(vec![]);

    inst.tracer.set_direction(100);
    inst.exec_command(
        &Command::Go { target: GoTarget::Value(str_lit("abc")), relative: false },
        &mut ctx,
    ).unwrap();

    assert_eq!(inst.tracer.direction, 100);
    assert!(!ctx.warnings.is_empty());
}

/* --- SPLIT --- */

#[test]
fn split_on_whitespace_collapses_runs() {
    let scope = test_scope();
    let mut inst = Instance::new(&scope, None, (1.0, 1.0), None);
    let mut ctx = test_ctx(vec![]);

    inst.exec_command(
        &Command::Split { value: str_lit("  a  bb   c "), over: None },
        &mut ctx,
    ).unwrap();

    assert_eq!(
        inst.state.buffer,
        vec![
            Value::Str(b"a".to_vec()),
            Value::Str(b"bb".to_vec()),
            Value::Str(b"c".to_vec()),
        ]
    );
}

#[test]
fn split_over_a_separator_keeps_empty_pieces() {
    let scope = test_scope();
    let mut inst = Instance::new(&scope, None, (1.0, 1.0), None);
    let mut ctx = test_ctx(vec![]);

    inst.exec_command(
        &Command::Split { value: str_lit("a,,b"), over: Some(str_lit(",")) },
        &mut ctx,
    ).unwrap();

    assert_eq!(
        inst.state.buffer,
        vec![
            Value::Str(b"a".to_vec()),
            Value::Str(b"".to_vec()),
            Value::Str(b"b".to_vec()),
        ]
    );
}

#[test]
fn split_over_a_multibyte_separator() {
    let scope = test_scope();
    let mut inst = Instance::new(&scope, None, (1.0, 1.0), None);
    let mut ctx = test_ctx(vec![]);

    inst.exec_command(
        &Command::Split { value: str_lit("a::b::c"), over: Some(str_lit("::")) },
        &mut ctx,
    ).unwrap();

    assert_eq!(inst.state.buffer.len(), 3);
    assert_eq!(inst.state.buffer[1], Value::Str(b"b".to_vec()));
}

#[test]
fn split_with_no_match_pushes_the_whole_string() {
    let scope = test_scope();
    let mut inst = Instance::new(&scope, None, (1.0, 1.0), None);
    let mut ctx = test_ctx(vec![]);

    inst.exec_command(
        &Command::Split { value: str_lit("abc"), over: Some(str_lit(",")) },
        &mut ctx,
    ).unwrap();

    assert_eq!(inst.state.buffer, vec![Value::Str(b"abc".to_vec())]);
}

#[test]
fn split_on_empty_separator_falls_back_to_whitespace() {
    // spec gap: an empty separator has no meaningful split points
    let scope = test_scope();
    let mut inst = Instance::new(&scope, None, (1.0, 1.0), None);
    let mut ctx = test_ctx(vec![]);

    inst.exec_command(
        &Command::Split { value: str_lit("a b"), over: Some(str_lit("")) },
        &mut ctx,
    ).unwrap();

    assert_eq!(inst.state.buffer.len(), 2);
}

/* --- value evaluation --- */

#[test]
fn dynamic_variable_names_resolve_unquoted() {
    // regression: Display quotes strings, so a dynamic name must go through the
    // spec's STRING cast rather than to_string()
    let scope = test_scope();
    let mut inst = Instance::new(&scope, None, (1.0, 1.0), None);
    let mut ctx = test_ctx(vec![]);

    inst.state.set_variable("counter".into(), Value::Int(9));

    let expr = ValueExpr::DynamicVar {
        name: Box::new(str_lit("counter")),
        cast: None,
    };
    assert_eq!(inst.eval(&expr, &mut ctx).unwrap(), Value::Int(9));
}

#[test]
fn dynamic_variable_names_work_as_write_targets() {
    let scope = test_scope();
    let mut inst = Instance::new(&scope, None, (1.0, 1.0), None);
    let mut ctx = test_ctx(vec![]);

    inst.exec_command(
        &Command::Store {
            source: StoreSource::Value(int(5)),
            target: Some(ValueExpr::DynamicVar {
                name: Box::new(str_lit("dyn")),
                cast: None,
            }),
        },
        &mut ctx,
    ).unwrap();

    assert_eq!(inst.state.get_variable("dyn"), Value::Int(5));
}

#[test]
fn unassigned_variables_are_null() {
    let scope = test_scope();
    let inst = Instance::new(&scope, None, (1.0, 1.0), None);
    let mut ctx = test_ctx(vec![]);

    assert_eq!(inst.eval(&var("nope"), &mut ctx).unwrap(), Value::Null);
}

#[test]
fn storing_an_uncastable_value_to_the_dataspace_warns_and_stores_zero() {
    let scope = test_scope();
    let mut inst = Instance::new(&scope, None, (1.0, 1.0), None);
    let mut ctx = test_ctx(vec![]);

    inst.state.set_current_cell(5);
    inst.exec_command(
        &Command::Store { source: StoreSource::Value(str_lit("abc")), target: None },
        &mut ctx,
    ).unwrap();

    assert_eq!(inst.state.current_cell(), 0);
    assert!(!ctx.warnings.is_empty());
}

/* --- step --- */

#[test]
fn step_advances_and_triggers_nodes() {
    // START at (1,1) facing east, a PRINT node at (3,1) with radius 1
    let scope = Scope {
        title: "T".into(),
        metadata: Metadata { width: 10, height: 10, data_width: 4, data_height: 4,
            radius: 1, steps: None, debug: DebugMode::False, seed: None },
        nodes: vec![
            Node { position: Position { x: 1, y: 1 }, command: Command::Start },
            Node { position: Position { x: 3, y: 1 }, command: Command::Print(PrintTarget::Value(str_lit("hit"))) },
        ],
        checkpoints: vec![],
    };
    let mut inst = Instance::new(&scope, None, (1.0, 1.0), None);
    let mut ctx = test_ctx(vec![]);

    // two steps east lands at x=3
    inst.step(&mut ctx).unwrap();
    inst.step(&mut ctx).unwrap();

    assert_eq!(ctx.captured_output().unwrap(), b"hit");
}

#[test]
fn step_exits_at_the_boundary() {
    let scope = test_scope(); // width 10
    let mut inst = Instance::new(&scope, None, (10.0, 1.0), None);
    let mut ctx = test_ctx(vec![]);

    // facing east at the eastern edge — one step leaves the rectangle
    let outcome = inst.step(&mut ctx).unwrap();
    assert_eq!(outcome, StepOutcome::Exited);
}

#[test]
fn steps_limit_throws_when_exceeded() {
    let scope = test_scope();
    let mut inst = Instance::new(&scope, Some(2), (1.0, 1.0), None);
    let mut ctx = test_ctx(vec![]);

    inst.step(&mut ctx).unwrap();
    inst.step(&mut ctx).unwrap();
    let result = inst.step(&mut ctx);

    assert!(matches!(result, Err(Error::StepsExceeded(2))));
}
