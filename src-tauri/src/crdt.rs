use std::time::SystemTime;

use difference::{Changeset, Difference};
use yrs::{Doc, GetString, Text, Transact};

#[derive(Debug, Clone)]
pub struct Event {
    operations: Vec<Operation>,
    timestamp_ms: u64,
}

#[derive(Debug, Clone)]
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

#[derive(Debug, Clone)]
pub struct TextDocument {
    doc: Doc,
    history: Vec<Event>,
}

const TEXT_DOCUMENT_NAME: &str = "document";

impl TextDocument {
    fn apply(doc: &Doc, events: &Vec<Event>) {
        if events.len() == 0 {
            return;
        }
        let text = doc.get_or_insert_text(TEXT_DOCUMENT_NAME);
        let mut txn = doc.transact_mut();
        for event in events {
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

    fn from_history(history: Vec<Event>) -> TextDocument {
        let doc = Doc::new();
        Self::apply(&doc, &history);
        TextDocument {
            doc: doc.clone(),
            history,
        }
    }

    pub fn from_string(value: &str) -> TextDocument {
        Self::from_history(vec![Event {
            operations: get_delta_operations("", value),
            timestamp_ms: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
        }])
    }

    pub fn update(&mut self, value: &str) {
        let diffs = get_delta_operations(&self.to_string(), value);
        let event = Event {
            operations: diffs,
            timestamp_ms: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_millis() as u64,
        };
        Self::apply(&self.doc, &vec![event.clone()]);
        self.history.push(event);
        println!("History: {:?}", self.history);
    }

    pub fn at(&self, timestamp_ms: u64) -> TextDocument {
        let mut events: Vec<Event> = vec![];
        for event in self.history.iter() {
            if event.timestamp_ms <= timestamp_ms {
                events.push(event.clone());
            }
        }
        Self::from_history(events)
    }

    pub fn to_string(&self) -> String {
        let doc = &self.doc;
        let text = doc.get_or_insert_text(TEXT_DOCUMENT_NAME);
        let txn = doc.transact();
        text.get_string(&txn)
    }
}
