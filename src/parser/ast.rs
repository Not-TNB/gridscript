use crate::types::{Value, DataType};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Position {
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DebugMode { True, False, Auto }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Checkpoint {
    pub position: Position,
    pub id: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    pub position: Position,
    pub command: Command,
}

#[derive(Debug, Clone, Default)]
pub struct RawMetadata {
    pub width: Option<i64>,
    pub height: Option<i64>,
    pub data_width: Option<i64>,
    pub data_height: Option<i64>,
    pub radius: Option<i64>,
    pub steps: Option<i64>,
    pub debug: Option<DebugMode>,
    pub seed: Option<u64>,
    pub max_depth: Option<i64>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ValueExpr {
    Literal(Value),
    Var(String),
    DynamicVar {
        name: Box<ValueExpr>,
        cast: Option<DataType>,
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum GotoTarget {
    Id(ValueExpr),
    ThisCheckpoint,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ArithOp { Increment, Decrement, Multiply, Divide }

#[derive(Debug, Clone, PartialEq)]
pub enum GoTarget {
    North, South, East, West,
    Random, ThisDirection,
    Value(ValueExpr),
}

#[derive(Debug, Clone, PartialEq)]
pub enum SwitchCond {
    Random,
    Truthy(ValueExpr),    // SWITCH value
    Falsy(ValueExpr),     // SWITCH !value
    Equals(ValueExpr),    // SWITCH =value
    NotEquals(ValueExpr), // SWITCH !=value
}

#[derive(Debug, Clone, PartialEq)]
pub enum StoreSource {
    Random,
    Value(ValueExpr),
}

#[derive(Debug, Clone, PartialEq)]
pub struct ToClause {
    pub cast: Option<DataType>,
    pub target: ValueExpr,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PrintTarget {
    DataCell, // bare print
    Newline,
    Image(ValueExpr),
    File(ValueExpr),
    Value(ValueExpr),
}

#[derive(Debug, Clone, PartialEq)]
pub enum RemoveFrom {
    Bottom, // bare REMOVE
    Top,
    AnyPosition,
    ThisPosition,
    Position(ValueExpr),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoveMode { To, By }

#[derive(Debug, Clone, PartialEq)]
pub enum Command {
    Start,
    Home,
    Shuffle,
    NextValue,
    PreviousValue,
    NextRow,
    PreviousRow,
    Throw(ValueExpr),
    Warn(ValueExpr),
    Arithmetic {
        op: ArithOp,
        target: Option<ValueExpr>,
        by: Option<ValueExpr>,
        giving: Option<ValueExpr>,
    },
    Push(Option<ValueExpr>),
    Goto(GotoTarget),
    Go { target: GoTarget, relative: bool },
    Switch(SwitchCond),
    Store { source: StoreSource, target: Option<ValueExpr> },
    Peek(Option<ToClause>),
    Split { value: ValueExpr, over: Option<ValueExpr> },
    Return(Option<ValueExpr>),
    Print(PrintTarget),
    Remove { from: RemoveFrom, to: Option<ToClause> },
    LoadFile { path: ValueExpr, to: Option<ToClause> },
    MoveLastNode { 
        mode: MoveMode, 
        x: ValueExpr, 
        y: ValueExpr 
    },
    Call { 
        name: String, 
        arguments: Vec<ValueExpr>, 
        giving: Option<ValueExpr> 
    },
}
