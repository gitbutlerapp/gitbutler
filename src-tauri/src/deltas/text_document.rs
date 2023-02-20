use std::time::SystemTime;
use yrs::{Doc, GetString, Text, Transact};

use super::{deltas, operations};

#[derive(Debug, Clone, Default)]
pub struct TextDocument {
    doc: Doc,
    deltas: Vec<deltas::Delta>,
}

const TEXT_DOCUMENT_NAME: &str = "document";

impl TextDocument {
    fn apply_deltas(doc: &Doc, deltas: &Vec<deltas::Delta>) {
        if deltas.len() == 0 {
            return;
        }
        let text = doc.get_or_insert_text(TEXT_DOCUMENT_NAME);
        let mut txn = doc.transact_mut();
        for event in deltas {
            for operation in event.operations.iter() {
                match operation {
                    operations::Operation::Insert((index, chunk)) => {
                        text.insert(&mut txn, *index, chunk);
                    }
                    operations::Operation::Delete((index, len)) => {
                        text.remove_range(&mut txn, *index, *len);
                    }
                }
            }
        }
    }

    // creates a new text document from a deltas.
    pub fn from_deltas(deltas: Vec<deltas::Delta>) -> TextDocument {
        let doc = Doc::new();
        Self::apply_deltas(&doc, &deltas);
        TextDocument {
            doc: doc.clone(),
            deltas,
        }
    }

    pub fn get_deltas(&self) -> Vec<deltas::Delta> {
        self.deltas.clone()
    }

    // returns a text document where internal state is seeded with value, and deltas are applied.
    pub fn new(value: &str, deltas: Vec<deltas::Delta>) -> TextDocument {
        let doc = Doc::new();
        let mut all_deltas = vec![deltas::Delta {
            operations: operations::get_delta_operations("", value),
            timestamp_ms: 0,
        }];
        all_deltas.append(&mut deltas.clone());
        Self::apply_deltas(&doc, &all_deltas);
        TextDocument {
            doc: doc.clone(),
            deltas,
        }
    }

    pub fn update(&mut self, value: &str) -> bool {
        let diffs = operations::get_delta_operations(&self.to_string(), value);
        let event = deltas::Delta {
            operations: diffs,
            timestamp_ms: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_millis(),
        };
        if event.operations.len() == 0 {
            return false;
        }
        Self::apply_deltas(&self.doc, &vec![event.clone()]);
        self.deltas.push(event);
        return true;
    }

    pub fn to_string(&self) -> String {
        let doc = &self.doc;
        let text = doc.get_or_insert_text(TEXT_DOCUMENT_NAME);
        let txn = doc.transact();
        text.get_string(&txn)
    }
}
