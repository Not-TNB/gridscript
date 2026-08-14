use gridscript::parser::ast::*;
use gridscript::parser::parse;
use gridscript::types::{DataType, Value};
use std::fs;

fn load(name: &str) -> String {
    fs::read_to_string(format!("examples/{name}")).unwrap()
}

fn int(n: i32) -> ValueExpr {
    ValueExpr::Literal(Value::Int(n))
}
fn var(s: &str) -> ValueExpr {
    ValueExpr::Var(s.into())
}
fn str_lit(s: &str) -> ValueExpr {
    ValueExpr::Literal(Value::Str(s.as_bytes().to_vec()))
}

#[test]
fn hello_world_parses() {
    let source = load("hello_world.gridscript");
    let program = parse(&source).unwrap();

    assert_eq!(program.main.title, "HELLO WORLD");
    assert!(program.subroutines.is_empty());
}

#[test]
fn truth_machine_1_parses() {
    let source = load("truth_machine_1.gridscript");
    let program = parse(&source).unwrap();

    assert_eq!(program.main.title, "TRUTH MACHINE");
    assert!(program.subroutines.is_empty());
}

#[test]
fn truth_machine_2_parses() {
    let source = load("truth_machine_2.gridscript");
    let program = parse(&source).unwrap();

    assert_eq!(program.main.title, "TRUTH MACHINE 2");
    assert_eq!(program.main.checkpoints.len(), 1);
    assert!(program.subroutines.is_empty());
}

#[test]
fn factorial_parses() {
    let source = load("factorial.gridscript");
    let program = parse(&source).unwrap();

    assert_eq!(program.main.title, "FACTORIAL");
    assert_eq!(program.main.checkpoints.len(), 1);
    assert!(program.subroutines.is_empty());
}

#[test]
fn ackermann_parses() {
    let source = load("ackermann.gridscript");
    let program = parse(&source).unwrap();

    assert_eq!(program.main.title, "ACKERMANN FUNCTION");
    assert_eq!(program.subroutines.len(), 1);
    assert!(program.subroutines.contains_key("ACK"));
    assert_eq!(program.subroutines["ACK"].nodes.len(), 15);
}

#[test]
fn word_processor_parses_exactly() {
    let source = load("word_processor.gridscript");
    let program = parse(&source).unwrap();

    assert_eq!(program.max_depth, 10);
    assert_eq!(program.subroutines.len(), 2);

    let main = &program.main;
    assert_eq!(main.title, "WORD PROCESSOR");
    assert_eq!(main.metadata.width, 30);
    assert_eq!(main.metadata.height, 10);
    assert_eq!(
        main.checkpoints,
        vec![Checkpoint {
            position: Position { x: 7, y: 1 },
            id: 0,
        }]
    );
    assert_eq!(
        main.nodes,
        vec![
            Node {
                position: Position { x: 1, y: 1 },
                command: Command::Start
            },
            Node {
                position: Position { x: 3, y: 1 },
                command: Command::Input {
                    cast: Some(DataType::Str),
                    target: Some(var("text")),
                    prompt: None,
                }
            },
            Node {
                position: Position { x: 5, y: 1 },
                command: Command::Split {
                    value: var("text"),
                    over: None,
                }
            },
            Node {
                position: Position { x: 9, y: 1 },
                command: Command::Remove {
                    from: RemoveFrom::Top,
                    to: Some(ToClause {
                        cast: Some(DataType::Str),
                        target: var("word")
                    }),
                }
            },
            Node {
                position: Position { x: 11, y: 1 },
                command: Command::Switch(SwitchCond::Truthy(var("word")))
            },
            Node {
                position: Position { x: 11, y: 3 },
                command: Command::Call {
                    name: "CLASSIFY".into(),
                    arguments: vec![var("word")],
                    giving: Some(var("label")),
                }
            },
            Node {
                position: Position { x: 13, y: 1 },
                command: Command::Print(PrintTarget::Value(var("label")))
            },
            Node {
                position: Position { x: 15, y: 1 },
                command: Command::Store {
                    source: StoreSource::Random,
                    target: Some(var("r")),
                }
            },
            Node {
                position: Position { x: 17, y: 1 },
                command: Command::Switch(SwitchCond::Truthy(ValueExpr::DynamicVar {
                    name: Box::new(str_lit("r")),
                    cast: None,
                }))
            },
            Node {
                position: Position { x: 17, y: 3 },
                command: Command::Shuffle
            },
            Node {
                position: Position { x: 19, y: 1 },
                command: Command::Go {
                    target: GoTarget::Value(int(5)),
                    relative: true,
                }
            },
            Node {
                position: Position { x: 21, y: 1 },
                command: Command::Goto(GotoTarget::Id(int(0)))
            },
            Node {
                position: Position { x: 23, y: 1 },
                command: Command::Peek(Some(ToClause {
                    cast: Some(DataType::Int),
                    target: var("leftover"),
                }))
            },
            Node {
                position: Position { x: 25, y: 1 },
                command: Command::Warn(str_lit("buffer not fully drained"))
            },
            Node {
                position: Position { x: 27, y: 1 },
                command: Command::Print(PrintTarget::Newline)
            },
        ]
    );

    let classify = &program.subroutines["CLASSIFY"];
    assert_eq!(
        classify.checkpoints,
        vec![Checkpoint {
            position: Position { x: 7, y: 1 },
            id: 1,
        }]
    );
    assert_eq!(
        classify.nodes,
        vec![
            Node {
                position: Position { x: 1, y: 1 },
                command: Command::Start
            },
            Node {
                position: Position { x: 3, y: 1 },
                command: Command::Input {
                    cast: Some(DataType::Str),
                    target: Some(var("w")),
                    prompt: None,
                }
            },
            Node {
                position: Position { x: 5, y: 1 },
                command: Command::Store {
                    source: StoreSource::Value(int(0)),
                    target: Some(var("len")),
                }
            },
            Node {
                position: Position { x: 9, y: 1 },
                command: Command::Switch(SwitchCond::Truthy(var("w")))
            },
            Node {
                position: Position { x: 9, y: 3 },
                command: Command::Throw(str_lit("empty word reached unexpectedly"))
            },
            Node {
                position: Position { x: 11, y: 1 },
                command: Command::Call {
                    name: "COUNT_LEN".into(),
                    arguments: vec![var("w")],
                    giving: Some(var("len")),
                }
            },
            Node {
                position: Position { x: 13, y: 1 },
                command: Command::Return(Some(var("len")))
            },
        ]
    );

    let count_len = &program.subroutines["COUNT_LEN"];
    assert!(count_len.checkpoints.is_empty());
    assert_eq!(
        count_len.nodes,
        vec![
            Node {
                position: Position { x: 1, y: 1 },
                command: Command::Start
            },
            Node {
                position: Position { x: 3, y: 1 },
                command: Command::Input {
                    cast: Some(DataType::Str),
                    target: Some(var("s")),
                    prompt: None,
                }
            },
            Node {
                position: Position { x: 5, y: 1 },
                command: Command::Return(Some(var("s")))
            },
        ]
    );
}
