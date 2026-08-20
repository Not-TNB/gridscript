use gridscript::interpreter::context::RunContext;
use gridscript::interpreter::io::{BufferIn, BufferOut};
use gridscript::rng::GridScriptRng;
use gridscript::{interpreter, parser};

/// Parses and runs `source` with scripted stdin, returning the run's result and
/// everything written to the program output.
fn run_capture(source: &str, stdin: &[&str]) -> (gridscript::error::Result<()>, String) {
    let program = parser::parse(source).expect("sample should parse");
    let mut ctx = RunContext::new(
        GridScriptRng::from_seed(0),
        Box::new(BufferOut::default()),
        Box::new(BufferIn::new(stdin.iter().map(|s| s.to_string()))),
    );
    let result = interpreter::run(&program, &mut ctx);
    let output = String::from_utf8_lossy(ctx.captured_output().unwrap_or_default()).into_owned();
    (result, output)
}

const HELLO: &str = include_str!("../examples/hello_world.gridscript");
const TRUTH: &str = include_str!("../examples/truth_machine_1.gridscript");
const FACTORIAL: &str = include_str!("../examples/factorial.gridscript");
const ACKERMANN: &str = include_str!("../examples/ackermann.gridscript");

#[test]
fn hello_world_prints_greeting() {
    let (result, output) = run_capture(HELLO, &[]);
    assert!(result.is_ok());
    assert_eq!(output, "Hello World");
}

#[test]
fn truth_machine_halts_on_zero() {
    let (result, output) = run_capture(TRUTH, &["0"]);
    assert!(result.is_ok());
    assert_eq!(output, "0");
}

#[test]
fn factorial_computes_small_inputs() {
    for (input, expected) in [("1", "1"), ("5", "120"), ("12", "479001600")] {
        let (result, output) = run_capture(FACTORIAL, &[input]);
        assert!(result.is_ok(), "{input}! should succeed");
        assert_eq!(output, expected, "{input}!");
    }
}

#[test]
fn factorial_overflows_past_twelve() {
    let (result, _) = run_capture(FACTORIAL, &["13"]);
    assert!(result.is_err(), "13! should overflow the 32-bit INT range");
}

#[test]
fn ackermann_recurses() {
    for (x, y, expected) in [("0", "5", "6"), ("2", "2", "7"), ("3", "3", "61")] {
        let (result, output) = run_capture(ACKERMANN, &[x, y]);
        assert!(result.is_ok(), "A({x},{y}) should succeed");
        assert_eq!(output, expected, "A({x},{y})");
    }
}

/// Regression: adjacent nodes are tangent at the default radius, so an inclusive
/// containment test fires both branch columns of ACK's SWITCH and collapses every
/// result to y+1.
#[test]
fn ackermann_does_not_take_both_branches() {
    let (_, output) = run_capture(ACKERMANN, &["2", "3"]);
    assert_eq!(output, "9");
}
