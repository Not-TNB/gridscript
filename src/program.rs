use std::collections::HashMap;
use crate::error::{GridScriptError, Result};
use crate::parser::ast::{Node, Checkpoint, DebugMode, RawMetadata};

#[derive(Debug, Copy, Clone)]
pub struct Metadata {
    pub width: i32,         // required
    pub height: i32,        // required
    pub data_width: i32,    // default 64
    pub data_height: i32,   // default 64
    pub radius: i32,        // default 1
    pub steps: Option<u64>, // default None = unlimited
    pub debug: DebugMode,   // default false
    pub seed: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct Scope {
    pub title: String,
    pub metadata: Metadata,
    pub nodes: Vec<Node>,
    pub checkpoints: Vec<Checkpoint>,
}

#[derive(Debug, Clone)]
pub struct Program {
    pub main: Scope,
    pub subroutines: HashMap<String, Scope>,
    pub max_depth: u32,
}

/* Helpers for Metadata::from_raw */

fn require_positive(raw: Option<i64>, key: &str) -> Result<i32> {
    let value = raw.ok_or_else(|| GridScriptError::MissingMetadata(key.into()))?;
    if value < 1 {
        return Err(GridScriptError::InvalidMetadata { key: key.into(), value });
    }
    Ok(value as i32)
}

fn positive_or_default(raw: Option<i64>, key: &str, default: i32) -> Result<i32> {
    match raw {
        None => Ok(default),
        Some(value) if value < 1 => Err(GridScriptError::InvalidMetadata {
            key: key.into(), value,
        }),
        Some(value) => Ok(value as i32),
    }
}

impl Metadata {
    pub fn from_raw(raw: RawMetadata) -> Result<Self> {
        let width = require_positive(raw.width, "width")?;
        let height = require_positive(raw.height, "height")?;

        let data_width = positive_or_default(raw.data_width, "datawidth", 64)?;
        let data_height = positive_or_default(raw.data_height, "dataheight", 64)?;
        let radius = positive_or_default(raw.radius, "radius", 1)?;

        let steps = match raw.steps {
            None => None,
            Some(value) if value < 1 => {
                return Err(GridScriptError::InvalidMetadata { key: "steps".into(), value });
            }
            Some(value) => Some(value as u64),
        };

        let debug = raw.debug.unwrap_or(DebugMode::False);

        let seed = raw.seed;

        Ok(Self {
            width, height,
            data_width, data_height, radius,
            steps, debug,
            seed,
        })
    }
}
