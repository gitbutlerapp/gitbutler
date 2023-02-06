use difference::{Changeset, Difference};
use serde::{Deserialize, Serialize};
use std::time::SystemTime;
use yrs::{Doc, GetString, Text, Transact};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Delta {
    operations: Vec<Operation>,
    timestamp_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Operation {
    // corresponds to YText.insert(index, chunk)
    Insert((u32, String)),
    // corresponds to YText.remove_range(index, len)
    Delete((u32, u32)),
}

fn get_delta_operations(initial_text: &str, final_text: &str) -> Vec<Operation> {
    if initial_text == final_text {
        return vec![];
    }

    let changeset = Changeset::new(initial_text, final_text, "");
    let mut offset: u32 = 0;
    let mut deltas = vec![];

    for edit in changeset.diffs {
        match edit {
            Difference::Rem(text) => {
                offset -= text.len() as u32;
                deltas.push(Operation::Delete((offset, text.len() as u32)));
            }
            Difference::Add(text) => {
                deltas.push(Operation::Insert((offset, text.clone())));
                offset += text.len() as u32;
            }
            Difference::Same(text) => {
                offset += text.len() as u32;
            }
        }
    }

    return deltas;
}

#[derive(Debug, Clone, Default)]
pub struct TextDocument {
    doc: Doc,
    deltas: Vec<Delta>,
}

const TEXT_DOCUMENT_NAME: &str = "document";

impl TextDocument {
    fn apply_deltas(doc: &Doc, deltas: &Vec<Delta>) {
        if deltas.len() == 0 {
            return;
        }
        let text = doc.get_or_insert_text(TEXT_DOCUMENT_NAME);
        let mut txn = doc.transact_mut();
        for event in deltas {
            for operation in event.operations.iter() {
                match operation {
                    Operation::Insert((index, chunk)) => {
                        text.insert(&mut txn, *index, chunk);
                    }
                    Operation::Delete((index, len)) => {
                        text.remove_range(&mut txn, *index, *len);
                    }
                }
            }
        }
    }

    // creates a new text document from a deltas.
    pub fn from_deltas(deltas: Vec<Delta>) -> TextDocument {
        let doc = Doc::new();
        Self::apply_deltas(&doc, &deltas);
        TextDocument {
            doc: doc.clone(),
            deltas,
        }
    }

    pub fn get_deltas(&self) -> Vec<Delta> {
        self.deltas.clone()
    }

    // returns a text document where internal state is seeded with value, and deltas are applied.
    pub fn new(value: &str, deltas: Vec<Delta>) -> TextDocument {
        let doc = Doc::new();
        let mut all_deltas = vec![Delta {
            operations: get_delta_operations("", value),
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
        let diffs = get_delta_operations(&self.to_string(), value);
        let event = Delta {
            operations: diffs,
            timestamp_ms: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
        };
        if event.operations.len() == 0 {
            return false;
        }
        Self::apply_deltas(&self.doc, &vec![event.clone()]);
        self.deltas.push(event);
        return true;
    }

    pub fn at(&self, timestamp_ms: u64) -> TextDocument {
        let mut events: Vec<Delta> = vec![];
        for event in self.deltas.iter() {
            if event.timestamp_ms <= timestamp_ms {
                events.push(event.clone());
            }
        }
        Self::from_deltas(events)
    }

    pub fn to_string(&self) -> String {
        let doc = &self.doc;
        let text = doc.get_or_insert_text(TEXT_DOCUMENT_NAME);
        let txn = doc.transact();
        text.get_string(&txn)
    }
}
