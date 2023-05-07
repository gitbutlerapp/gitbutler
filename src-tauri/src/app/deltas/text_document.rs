use super::{delta, operations};
use anyhow::Result;
use std::time::SystemTime;

#[derive(Debug, Clone, Default)]
pub struct TextDocument {
    doc: Vec<char>,
    deltas: Vec<delta::Delta>,
}

fn apply_deltas(doc: &mut Vec<char>, deltas: &Vec<delta::Delta>) -> Result<()> {
    for delta in deltas {
        for operation in &delta.operations {
            operation.apply(doc)?;
        }
    }
    Ok(())
}

impl TextDocument {
    pub fn get_deltas(&self) -> Vec<delta::Delta> {
        self.deltas.clone()
    }

    // returns a text document where internal state is seeded with value, and deltas are applied.
    pub fn new(value: Option<&str>, deltas: Vec<delta::Delta>) -> Result<TextDocument> {
        let mut all_deltas = vec![];
        if let Some(value) = value {
            all_deltas.push(delta::Delta {
                operations: operations::get_delta_operations("", value),
                timestamp_ms: 0,
            });
        }
        all_deltas.append(&mut deltas.clone());
        let mut doc = vec![];
        apply_deltas(&mut doc, &all_deltas)?;
        Ok(TextDocument { doc, deltas })
    }

    pub fn update(&mut self, value: &str) -> Result<bool> {
        let diffs = operations::get_delta_operations(&self.to_string(), value);
        let event = delta::Delta {
            operations: diffs,
            timestamp_ms: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_millis(),
        };
        if event.operations.len() == 0 {
            return Ok(false);
        }
        apply_deltas(&mut self.doc, &vec![event.clone()])?;
        self.deltas.push(event);
        return Ok(true);
    }

    pub fn to_string(&self) -> String {
        self.doc.iter().collect()
    }
}
