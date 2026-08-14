use crate::interpreter::tracer::DataTracer;
use crate::types::Value;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub struct State {
    dataspace: Vec<i32>,
    width: usize,
    height: usize,

    pub data_tracer: DataTracer,
    pub buffer: Vec<Value>,
    pub variables: HashMap<String, Value>,
}

impl State {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            dataspace: vec![0; width * height],
            width,
            height,
            data_tracer: DataTracer::new(),
            buffer: Vec::new(),
            variables: HashMap::new(),
        }
    }

    fn idx(&self) -> usize {
        self.data_tracer.row * self.width + self.data_tracer.col
    }
    pub fn current_cell(&self) -> i32 {
        self.dataspace[self.idx()]
    }
    pub fn set_current_cell(&mut self, value: i32) {
        let idx = self.idx();
        self.dataspace[idx] = value;
    }

    pub fn home(&mut self) {
        self.data_tracer.home();
    }
    pub fn next_value(&mut self) {
        self.data_tracer.next_value(self.width, self.height);
    }
    pub fn previous_value(&mut self) {
        self.data_tracer.previous_value(self.width, self.height);
    }
    pub fn next_row(&mut self) {
        self.data_tracer.next_row(self.height);
    }
    pub fn previous_row(&mut self) {
        self.data_tracer.previous_row(self.height);
    }

    /// Obtains the value of the variable with the given name.
    /// Gives the NULL value if no variable has this name.
    pub fn get_variable(&self, name: &str) -> Value {
        self.variables.get(name).cloned().unwrap_or(Value::Null)
    }

    /// Inserts or overwrites a variable with the given name.
    /// PRE: `name` is a valid variable identifier
    pub fn set_variable(&mut self, name: String, value: Value) {
        self.variables.insert(name, value);
    }

    /// PUSH; append value to buffer top
    pub fn push(&mut self, value: Value) {
        self.buffer.push(value);
    }
    /// REMOVE; remove and return buffer bottom
    pub fn remove_bottom(&mut self) -> Option<Value> {
        if self.buffer.is_empty() {
            return None;
        }
        Some(self.buffer.remove(0))
    }
    /// REMOVE TOP; remove and return buffer top
    pub fn remove_top(&mut self) -> Option<Value> {
        self.buffer.pop()
    }
    /// REMOVE position / REMOVE THIS position; remove and return value at given index in buffer
    pub fn remove_at(&mut self, index: usize) -> Option<Value> {
        if self.buffer.len() <= index {
            return None;
        }
        Some(self.buffer.remove(index))
    }
    /// PEEK; return copy of buffer bottom without removing
    pub fn peek(&self) -> Option<Value> {
        self.buffer.first().cloned()
    }
}
