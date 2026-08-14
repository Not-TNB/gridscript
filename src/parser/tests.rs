use super::*;

/// Parses a source snippet as a command.
fn cmd(source: &str) -> Result<Command> {
    let tokens = lexer::tokenize(source)?;
    Parser::new(&tokens).parse_command()
}

/// Parses a source snippet as a value expression.
fn expr(source: &str) -> Result<ValueExpr> {
    let tokens = lexer::tokenize(source)?;
    Parser::new(&tokens).parse_value_expr()
}

/// Parses a source snippet as a metadata block.
fn meta(source: &str) -> Result<RawMetadata> {
    let tokens = lexer::tokenize(source)?;
    Parser::new(&tokens).parse_metadata()
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

/* --- titles --- */

#[test]
fn splits_titles() {
    let (title, rest) = split_title("#HELLO WORLD.\n\n@width 4", 1).unwrap();
    assert_eq!(title, "HELLO WORLD");
    assert_eq!(rest, "\n@width 4");

    assert!(split_title("##ACK.\n", 1).is_err());
    assert!(split_title("#NO PERIOD\n", 1).is_err());
}

/* --- metadata --- */

#[test]
fn parses_metadata_block() {
    let raw = meta("\n@width 4\n@height 1\n@debug auto\n").unwrap();
    assert_eq!(raw.width, Some(4));
    assert_eq!(raw.height, Some(1));
    assert_eq!(raw.debug, Some(DebugMode::Auto));

    assert!(meta("@bogus 5\n").is_err());
}

/* --- value expressions --- */

#[test]
fn parses_literals_and_variables() {
    assert_eq!(expr("42").unwrap(), int(42));
    assert_eq!(expr("-7").unwrap(), int(-7));
    assert_eq!(expr("3.5").unwrap(), ValueExpr::Literal(Value::Float(3.5)));
    assert_eq!(expr("'hi'").unwrap(), str_lit("hi"));
    assert_eq!(expr("TRUE").unwrap(), ValueExpr::Literal(Value::Bool(true)));
    assert_eq!(expr("count_2").unwrap(), var("count_2"));
}

#[test]
fn parses_dynamic_variables() {
    assert_eq!(
        expr("THE VARIABLE NAMED 'x'").unwrap(),
        ValueExpr::DynamicVar {
            name: Box::new(str_lit("x")),
            cast: None,
        }
    );
    assert_eq!(
        expr("THE INT NAMED holder").unwrap(),
        ValueExpr::DynamicVar {
            name: Box::new(var("holder")),
            cast: Some(DataType::Int),
        }
    );
    // nesting exercises the recursion and the Box
    assert_eq!(
        expr("THE VARIABLE NAMED THE STRING NAMED 'outer'").unwrap(),
        ValueExpr::DynamicVar {
            name: Box::new(ValueExpr::DynamicVar {
                name: Box::new(str_lit("outer")),
                cast: Some(DataType::Str),
            }),
            cast: None,
        }
    );
}

#[test]
fn rejects_malformed_value_expressions() {
    assert!(expr("THE VARIABLE 'x'").is_err()); // missing NAMED
    assert!(expr("THE NAMED 'x'").is_err()); // bad slot after THE
    assert!(expr("THE").is_err()); // nothing after THE
    assert!(expr(":").is_err());
    assert!(expr("").is_err());
}

/* --- commands --- */

#[test]
fn parses_zero_argument_commands() {
    let cases = [
        ("START", Command::Start),
        ("HOME", Command::Home),
        ("SHUFFLE", Command::Shuffle),
        ("NEXT VALUE", Command::NextValue),
        ("NEXT ROW", Command::NextRow),
        ("PREVIOUS VALUE", Command::PreviousValue),
        ("PREVIOUS ROW", Command::PreviousRow),
    ];
    for (source, expected) in cases {
        assert_eq!(cmd(source).unwrap(), expected);
    }
    assert!(cmd("NEXT").is_err());
    assert!(cmd("NEXT HOME").is_err());
}

#[test]
fn parses_single_value_commands() {
    assert_eq!(
        cmd("THROW 'boom'").unwrap(),
        Command::Throw(str_lit("boom"))
    );
    assert_eq!(cmd("WARN x").unwrap(), Command::Warn(var("x")));
    assert_eq!(cmd("PUSH 5").unwrap(), Command::Push(Some(int(5))));
    assert_eq!(cmd("PUSH").unwrap(), Command::Push(None));
    assert!(cmd("THROW").is_err());
}

#[test]
fn parses_goto() {
    assert_eq!(
        cmd("GOTO 0").unwrap(),
        Command::Goto(GotoTarget::Id(int(0)))
    );
    assert_eq!(
        cmd("GOTO THIS CHECKPOINT").unwrap(),
        Command::Goto(GotoTarget::ThisCheckpoint)
    );
    assert!(cmd("GOTO THIS").is_err());
    assert!(cmd("GOTO THIS ROW").is_err());
}

#[test]
fn parses_arithmetic() {
    assert_eq!(
        cmd("INCREMENT").unwrap(),
        Command::Arithmetic {
            op: ArithOp::Increment,
            target: None,
            by: None,
            giving: None,
        }
    );
    assert_eq!(
        cmd("INCREMENT x BY 5 GIVING y").unwrap(),
        Command::Arithmetic {
            op: ArithOp::Increment,
            target: Some(var("x")),
            by: Some(int(5)),
            giving: Some(var("y")),
        }
    );
    // BY in the target slot must not be eaten as a target
    assert!(matches!(
        cmd("INCREMENT BY 5").unwrap(),
        Command::Arithmetic {
            target: None,
            by: Some(_),
            ..
        }
    ));

    assert!(cmd("MULTIPLY x").is_err());
    assert!(cmd("MULTIPLY x BY 2").is_ok());
}

#[test]
fn parses_go() {
    let cases = [
        ("GO NORTH", GoTarget::North, false),
        ("GO SOUTH", GoTarget::South, false),
        ("GO EAST", GoTarget::East, false),
        ("GO WEST", GoTarget::West, false),
        ("GO RANDOM", GoTarget::Random, false),
        ("GO THIS DIRECTION", GoTarget::ThisDirection, false),
        (
            "GO RELATIVE TO THIS DIRECTION",
            GoTarget::ThisDirection,
            true,
        ),
        ("GO 45", GoTarget::Value(int(45)), false),
        ("GO RELATIVE TO 90", GoTarget::Value(int(90)), true),
    ];
    for (source, target, relative) in cases {
        assert_eq!(cmd(source).unwrap(), Command::Go { target, relative });
    }

    assert!(cmd("GO RELATIVE NORTH").is_err());
    assert!(cmd("GO RELATIVE TO NORTH").is_err());
    assert!(cmd("GO THIS ROW").is_err());
}

#[test]
fn parses_switch() {
    let cases = [
        ("SWITCH RANDOM", SwitchCond::Random),
        ("SWITCH 5", SwitchCond::Truthy(int(5))),
        ("SWITCH !5", SwitchCond::Falsy(int(5))),
        ("SWITCH =5", SwitchCond::Equals(int(5))),
        ("SWITCH !=5", SwitchCond::NotEquals(int(5))),
    ];
    for (source, expected) in cases {
        assert_eq!(cmd(source).unwrap(), Command::Switch(expected));
    }
    // operator forms compose with the full value grammar
    assert!(matches!(
        cmd("SWITCH =THE INT NAMED 'n'").unwrap(),
        Command::Switch(SwitchCond::Equals(ValueExpr::DynamicVar { .. }))
    ));

    assert!(cmd("SWITCH").is_err());
    assert!(cmd("SWITCH =").is_err());
}

#[test]
fn parses_store() {
    assert_eq!(
        cmd("STORE 5").unwrap(),
        Command::Store {
            source: StoreSource::Value(int(5)),
            target: None,
        }
    );
    assert_eq!(
        cmd("STORE RANDOM TO x").unwrap(),
        Command::Store {
            source: StoreSource::Random,
            target: Some(var("x")),
        }
    );
}

/* --- cursor behaviour --- */

#[test]
fn stops_at_line_end_without_consuming() {
    let tokens = lexer::tokenize("PUSH\nHOME").unwrap();
    let mut p = Parser::new(&tokens);
    assert_eq!(p.parse_command().unwrap(), Command::Push(None));
    assert_eq!(p.peek(), Some(&Token::Newline));
}

#[test]
fn leaves_trailing_tokens_unconsumed() {
    let tokens = lexer::tokenize("5 GIVING x").unwrap();
    let mut p = Parser::new(&tokens);
    assert_eq!(p.parse_value_expr().unwrap(), int(5));
    assert_eq!(p.peek(), Some(&Token::Keyword(Keyword::Giving)));
}

#[test]
fn parses_peek() {
    assert_eq!(cmd("PEEK").unwrap(), Command::Peek(None));
    assert_eq!(
        cmd("PEEK TO x").unwrap(),
        Command::Peek(Some(ToClause {
            cast: None,
            target: var("x"),
        }))
    );
    assert_eq!(
        cmd("PEEK TO INT x").unwrap(),
        Command::Peek(Some(ToClause {
            cast: Some(DataType::Int),
            target: var("x"),
        }))
    );
    assert!(cmd("PEEK TO").is_err());
}

#[test]
fn parses_split_and_return() {
    assert_eq!(
        cmd("SPLIT 'a b'").unwrap(),
        Command::Split {
            value: str_lit("a b"),
            over: None,
        }
    );
    assert_eq!(
        cmd("SPLIT s OVER ','").unwrap(),
        Command::Split {
            value: var("s"),
            over: Some(str_lit(",")),
        }
    );
    assert_eq!(cmd("RETURN").unwrap(), Command::Return(None));
    assert_eq!(cmd("RETURN 5").unwrap(), Command::Return(Some(int(5))));
    assert!(cmd("SPLIT").is_err());
}

#[test]
fn parses_print() {
    assert_eq!(cmd("PRINT").unwrap(), Command::Print(PrintTarget::DataCell));
    assert_eq!(
        cmd("PRINT NEWLINE").unwrap(),
        Command::Print(PrintTarget::Newline)
    );
    assert_eq!(
        cmd("PRINT 'hi'").unwrap(),
        Command::Print(PrintTarget::Value(str_lit("hi")))
    );
    assert_eq!(
        cmd("PRINT FILE 'a.txt'").unwrap(),
        Command::Print(PrintTarget::File(str_lit("a.txt")))
    );
    assert_eq!(
        cmd("PRINT IMAGE p").unwrap(),
        Command::Print(PrintTarget::Image(var("p")))
    );
}

#[test]
fn parses_remove() {
    assert_eq!(
        cmd("REMOVE").unwrap(),
        Command::Remove {
            from: RemoveFrom::Bottom,
            to: None
        }
    );
    assert_eq!(
        cmd("REMOVE TOP").unwrap(),
        Command::Remove {
            from: RemoveFrom::Top,
            to: None
        }
    );
    assert_eq!(
        cmd("REMOVE ANY POSITION").unwrap(),
        Command::Remove {
            from: RemoveFrom::AnyPosition,
            to: None
        }
    );
    assert_eq!(
        cmd("REMOVE THIS POSITION").unwrap(),
        Command::Remove {
            from: RemoveFrom::ThisPosition,
            to: None
        }
    );
    assert_eq!(
        cmd("REMOVE 3").unwrap(),
        Command::Remove {
            from: RemoveFrom::Position(int(3)),
            to: None
        }
    );

    // bare REMOVE followed directly by TO -- the guard's reason for existing
    assert_eq!(
        cmd("REMOVE TO INT x").unwrap(),
        Command::Remove {
            from: RemoveFrom::Bottom,
            to: Some(ToClause {
                cast: Some(DataType::Int),
                target: var("x")
            }),
        }
    );

    assert!(cmd("REMOVE ANY").is_err());
    assert!(cmd("REMOVE THIS ROW").is_err());
}

#[test]
fn parses_load_file() {
    assert_eq!(
        cmd("LOAD FILE 'a.txt'").unwrap(),
        Command::LoadFile {
            path: str_lit("a.txt"),
            to: None,
        }
    );
    assert_eq!(
        cmd("LOAD FILE p TO STRING s").unwrap(),
        Command::LoadFile {
            path: var("p"),
            to: Some(ToClause {
                cast: Some(DataType::Str),
                target: var("s")
            }),
        }
    );

    assert!(cmd("LOAD 'a.txt'").is_err()); // missing FILE
    assert!(cmd("LOAD FILE").is_err());
}

#[test]
fn parses_move_last_node() {
    assert_eq!(
        cmd("MOVE LAST NODE TO 3 4").unwrap(),
        Command::MoveLastNode {
            mode: MoveMode::To,
            x: int(3),
            y: int(4),
        }
    );
    assert_eq!(
        cmd("MOVE LAST NODE BY -1 2").unwrap(),
        Command::MoveLastNode {
            mode: MoveMode::By,
            x: int(-1),
            y: int(2),
        }
    );

    assert!(cmd("MOVE LAST NODE 3 4").is_err()); // missing TO/BY
    assert!(cmd("MOVE NODE TO 3 4").is_err()); // missing LAST
    assert!(cmd("MOVE LAST NODE TO 3").is_err()); // missing y
}

#[test]
fn parses_call() {
    assert_eq!(
        cmd("CALL ACK").unwrap(),
        Command::Call {
            name: "ACK".into(),
            arguments: vec![],
            giving: None,
        }
    );
    assert_eq!(
        cmd("CALL ACK WITH ARGUMENTS x y GIVING z").unwrap(),
        Command::Call {
            name: "ACK".into(),
            arguments: vec![var("x"), var("y")],
            giving: Some(var("z")),
        }
    );
    assert_eq!(
        cmd("CALL FOO WITH ARGUMENTS 'Foo' 17").unwrap(),
        Command::Call {
            name: "FOO".into(),
            arguments: vec![str_lit("Foo"), int(17)],
            giving: None,
        }
    );
    assert_eq!(
        cmd("CALL FOO GIVING x").unwrap(),
        Command::Call {
            name: "FOO".into(),
            arguments: vec![],
            giving: Some(var("x")),
        }
    );

    // a multi-token argument counts as one argument, not four
    assert_eq!(
        cmd("CALL FOO WITH ARGUMENTS THE INT NAMED 'n' 5").unwrap(),
        Command::Call {
            name: "FOO".into(),
            arguments: vec![
                ValueExpr::DynamicVar {
                    name: Box::new(str_lit("n")),
                    cast: Some(DataType::Int),
                },
                int(5),
            ],
            giving: None,
        }
    );

    assert!(cmd("CALL PRINT").is_err()); // reserved word as name
    assert!(cmd("CALL x").is_err()); // lowercase name
    assert!(cmd("CALL").is_err());
    assert!(cmd("CALL FOO WITH x").is_err()); // missing ARGUMENTS
}

#[test]
fn call_arguments_stop_at_line_end() {
    let tokens = lexer::tokenize("CALL FOO WITH ARGUMENTS x\nHOME").unwrap();
    let mut p = Parser::new(&tokens);
    assert_eq!(
        p.parse_command().unwrap(),
        Command::Call {
            name: "FOO".into(),
            arguments: vec![var("x")],
            giving: None,
        }
    );
    assert_eq!(p.peek(), Some(&Token::Newline));
}

#[test]
fn parses_node_lines() {
    let tokens = lexer::tokenize("(3,1):PRINT 'hi'").unwrap();
    match Parser::new(&tokens).parse_body_line().unwrap() {
        BodyLine::Node(n) => {
            assert_eq!(n.position, Position { x: 3, y: 1 });
            assert_eq!(n.command, Command::Print(PrintTarget::Value(str_lit("hi"))));
        }
        _ => panic!("expected a node"),
    }

    let tokens = lexer::tokenize("(7,1):CHECKPOINT 0").unwrap();
    match Parser::new(&tokens).parse_body_line().unwrap() {
        BodyLine::Checkpoint(c) => {
            assert_eq!(c.position, Position { x: 7, y: 1 });
            assert_eq!(c.id, 0);
        }
        _ => panic!("expected a checkpoint"),
    }
}

#[test]
fn rejects_bad_node_lines() {
    let bad = [
        "3,1:PRINT",
        "(3 1):PRINT",
        "(3,1) PRINT",
        "(3,1):CHECKPOINT -1",
    ];
    for source in bad {
        let tokens = lexer::tokenize(source).unwrap();
        assert!(Parser::new(&tokens).parse_body_line().is_err());
    }
}

/* --- scope splitting --- */

#[test]
fn splits_zero_subroutines() {
    let source = "#HELLO.\n\n@width 4\n@height 1\n\n(1,1):START\n";
    let (main, subs) = split_scopes(source);
    assert_eq!(main, source);
    assert!(subs.is_empty());
}

#[test]
fn splits_one_subroutine() {
    let source = "#MAIN.\n\n@width 4\n@height 1\n\n(1,1):START\n\n##SUB.\n\n@width 2\n@height 2\n\n(1,1):START\n";
    let (main, subs) = split_scopes(source);

    assert_eq!(main, "#MAIN.\n\n@width 4\n@height 1\n\n(1,1):START\n\n");
    assert_eq!(subs.len(), 1);
    assert!(subs[0].starts_with("##SUB.\n"));
    assert!(subs[0].ends_with("(1,1):START\n"));
}

#[test]
fn splits_multiple_subroutines() {
    let source = "#MAIN.\n\n@width 1\n@height 1\n\n(1,1):START\n\n\
                  ##A.\n\n@width 1\n@height 1\n\n(1,1):START\n\n\
                  ##B.\n\n@width 1\n@height 1\n\n(1,1):START\n";
    let (main, subs) = split_scopes(source);

    assert!(main.starts_with("#MAIN."));
    assert_eq!(subs.len(), 2);
    assert!(subs[0].starts_with("##A."));
    assert!(subs[0].contains("##A.") && !subs[0].contains("##B."));
    assert!(subs[1].starts_with("##B."));
}

#[test]
fn ignores_double_octothorpe_not_at_line_start() {
    // a `##` appearing mid-line (e.g. inside output text) must not be treated
    // as a scope boundary
    let source = "#MAIN.\n\n@width 4\n@height 1\n\n(1,1):PRINT '## not a title'\n(3,1):START\n";
    let (main, subs) = split_scopes(source);

    assert_eq!(main, source);
    assert!(subs.is_empty());
}

#[test]
fn no_content_lost_across_boundaries() {
    let source = "#MAIN.\nA\n\n##SUB.\nB\n";
    let (main, subs) = split_scopes(source);

    // concatenating everything back together should reconstruct the source exactly
    let mut rebuilt = main.to_string();
    for s in &subs {
        rebuilt.push_str(s);
    }
    assert_eq!(rebuilt, source);
}

/* --- full program parsing --- */

fn program(source: &str) -> Result<Program> {
    parse(source)
}

#[test]
fn parses_program_with_no_subroutines() {
    let source = "#HELLO.\n\n@width 4\n@height 1\n\n(1,1):START\n(3,1):PRINT 'hi'\n";
    let p = program(source).unwrap();

    assert_eq!(p.main.title, "HELLO");
    assert_eq!(p.main.nodes.len(), 2);
    assert!(p.subroutines.is_empty());
    assert_eq!(p.max_depth, 1000); // default
}

#[test]
fn parses_program_with_one_subroutine() {
    let source = "\
#MAIN.

@width 4
@height 1

(1,1):START
(3,1):CALL SUB

##SUB.

@width 2
@height 1

(1,1):START
";
    let p = program(source).unwrap();

    assert_eq!(p.main.title, "MAIN");
    assert_eq!(p.subroutines.len(), 1);
    assert!(p.subroutines.contains_key("SUB"));
    assert_eq!(p.subroutines["SUB"].metadata.width, 2);
}

#[test]
fn main_maxdepth_is_applied() {
    let source = "#MAIN.\n\n@width 1\n@height 1\n@maxdepth 50\n\n(1,1):START\n";
    let p = program(source).unwrap();
    assert_eq!(p.max_depth, 50);
}

#[test]
fn subroutine_maxdepth_is_ignored() {
    let source = "\
#MAIN.

@width 1
@height 1
@maxdepth 7

(1,1):START

##SUB.

@width 1
@height 1
@maxdepth 999

(1,1):START
";
    let p = program(source).unwrap();
    // main's value wins; subroutine's @maxdepth has no effect on it
    assert_eq!(p.max_depth, 7);
}

#[test]
fn invalid_main_maxdepth_errors() {
    let source = "#MAIN.\n\n@width 1\n@height 1\n@maxdepth 0\n\n(1,1):START\n";
    assert!(program(source).is_err());
}

#[test]
fn duplicate_subroutine_titles_error() {
    let source = "\
#MAIN.

@width 1
@height 1

(1,1):START

##SUB.

@width 1
@height 1

(1,1):START

##SUB.

@width 1
@height 1

(1,1):START
";
    match program(source) {
        Err(Error::DuplicateSubroutine(title)) => assert_eq!(title, "SUB"),
        other => panic!("expected DuplicateSubroutine, got {other:?}"),
    }
}

#[test]
fn missing_start_in_subroutine_reports_subroutine_title() {
    let source = "\
#MAIN.

@width 1
@height 1

(1,1):START

##SUB.

@width 1
@height 1

(1,1):PRINT 'no start here'
";
    match program(source) {
        Err(Error::MissingStart(title)) => assert_eq!(title, "SUB"),
        other => panic!("expected MissingStart(\"SUB\"), got {other:?}"),
    }
}

#[test]
fn malformed_body_line_in_subroutine_propagates() {
    let source = "\
#MAIN.

@width 1
@height 1

(1,1):START

##SUB.

@width 1
@height 1

(1,1):BOGUS
";
    assert!(program(source).is_err());
}

#[test]
fn parses_dispatcher_program_exactly() {
    let source = "\
#DISPATCHER.

@width 12
@height 6
@maxdepth 25

(1,1):START
(3,1):CHECKPOINT 0
(5,1):INPUT INT TO n
(7,1):SWITCH =0
(7,3):CALL DOUBLE WITH ARGUMENTS n GIVING result
(9,1):CALL DESCRIBE WITH ARGUMENTS result
(11,1):PRINT NEWLINE

##DOUBLE.

@width 6
@height 1

(1,1):START
(3,1):INPUT INT TO x
(5,1):MULTIPLY x BY 2 GIVING x
(6,1):RETURN x

##DESCRIBE.

@width 8
@height 3

(1,1):START
(3,1):INPUT TO v
(5,1):PEEK TO INT p
(7,1):REMOVE 0 TO STRING s
";

    let program = parse(source).unwrap();

    assert_eq!(program.max_depth, 25);
    assert_eq!(program.subroutines.len(), 2);

    let main = &program.main;
    assert_eq!(main.title, "DISPATCHER");
    assert_eq!(main.metadata.width, 12);
    assert_eq!(main.metadata.height, 6);
    assert_eq!(
        main.checkpoints,
        vec![Checkpoint {
            position: Position { x: 3, y: 1 },
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
                position: Position { x: 5, y: 1 },
                command: Command::Input {
                    cast: Some(DataType::Int),
                    target: Some(var("n")),
                    prompt: None,
                }
            },
            Node {
                position: Position { x: 7, y: 1 },
                command: Command::Switch(SwitchCond::Equals(int(0)))
            },
            Node {
                position: Position { x: 7, y: 3 },
                command: Command::Call {
                    name: "DOUBLE".into(),
                    arguments: vec![var("n")],
                    giving: Some(var("result")),
                }
            },
            Node {
                position: Position { x: 9, y: 1 },
                command: Command::Call {
                    name: "DESCRIBE".into(),
                    arguments: vec![var("result")],
                    giving: None,
                }
            },
            Node {
                position: Position { x: 11, y: 1 },
                command: Command::Print(PrintTarget::Newline)
            },
        ]
    );

    let double = &program.subroutines["DOUBLE"];
    assert_eq!(
        double.nodes,
        vec![
            Node {
                position: Position { x: 1, y: 1 },
                command: Command::Start
            },
            Node {
                position: Position { x: 3, y: 1 },
                command: Command::Input {
                    cast: Some(DataType::Int),
                    target: Some(var("x")),
                    prompt: None,
                }
            },
            Node {
                position: Position { x: 5, y: 1 },
                command: Command::Arithmetic {
                    op: ArithOp::Multiply,
                    target: Some(var("x")),
                    by: Some(int(2)),
                    giving: Some(var("x")),
                }
            },
            Node {
                position: Position { x: 6, y: 1 },
                command: Command::Return(Some(var("x")))
            },
        ]
    );

    let describe = &program.subroutines["DESCRIBE"];
    assert_eq!(
        describe.nodes,
        vec![
            Node {
                position: Position { x: 1, y: 1 },
                command: Command::Start
            },
            Node {
                position: Position { x: 3, y: 1 },
                command: Command::Input {
                    cast: None,
                    target: Some(var("v")),
                    prompt: None,
                }
            },
            Node {
                position: Position { x: 5, y: 1 },
                command: Command::Peek(Some(ToClause {
                    cast: Some(DataType::Int),
                    target: var("p"),
                }))
            },
            Node {
                position: Position { x: 7, y: 1 },
                command: Command::Remove {
                    from: RemoveFrom::Position(int(0)),
                    to: Some(ToClause {
                        cast: Some(DataType::Str),
                        target: var("s")
                    }),
                }
            },
        ]
    );
}
