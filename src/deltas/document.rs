use crate::reader;

use super::{delta, operations};
use anyhow::Result;
use std::{
    fmt::{Display, Formatter},
    time::SystemTime,
};

#[derive(Debug, Clone, Default)]
pub struct Document {
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

impl Document {
    pub fn get_deltas(&self) -> Vec<delta::Delta> {
        self.deltas.clone()
    }

    // returns a text document where internal state is seeded with value, and deltas are applied.
    pub fn new(value: Option<&reader::Content>, deltas: Vec<delta::Delta>) -> Result<Document> {
        let mut all_deltas = vec![];
        if let Some(reader::Content::UTF8(value)) = value {
            all_deltas.push(delta::Delta {
                operations: operations::get_delta_operations("", value),
                timestamp_ms: 0,
            });
        }
        all_deltas.append(&mut deltas.clone());
        let mut doc = vec![];
        apply_deltas(&mut doc, &all_deltas)?;
        Ok(Document { doc, deltas })
    }

    pub fn update(&mut self, value: Option<&reader::Content>) -> Result<Option<delta::Delta>> {
        let new_text = match value {
            Some(reader::Content::UTF8(value)) => value,
            Some(_) | None => "",
        };

        let operations = operations::get_delta_operations(&self.to_string(), new_text);
        let delta = if operations.is_empty() {
            if let Some(reader::Content::UTF8(value)) = value {
                if !value.is_empty() {
                    return Ok(None);
                }
            }

            delta::Delta {
                operations,
                timestamp_ms: SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap()
                    .as_millis(),
            }
        } else {
            delta::Delta {
                operations,
                timestamp_ms: SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap()
                    .as_millis(),
            }
        };
        apply_deltas(&mut self.doc, &vec![delta.clone()])?;
        self.deltas.push(delta.clone());
        Ok(Some(delta))
    }
}

impl Display for Document {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.doc.iter().collect::<String>())
    }
}
