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
            Some(_) => "",
            None => "",
        };

        let operations = operations::get_delta_operations(&self.to_string(), new_text);
        let delta = if operations.is_empty() {
            if matches!(value, Some(reader::Content::UTF8(_))) {
                return Ok(None);
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

#[cfg(test)]
mod tests {
    use self::{delta::Delta, operations::Operation};

    use super::*;

    #[test]
    fn test_new() {
        let document = Document::new(
            Some(&reader::Content::UTF8("hello world".to_string())),
            vec![],
        );
        assert!(document.is_ok());
        let document = document.unwrap();
        assert_eq!(document.to_string(), "hello world");
        assert_eq!(document.get_deltas().len(), 0);
    }

    #[test]
    fn test_update() {
        let document = Document::new(
            Some(&reader::Content::UTF8("hello world".to_string())),
            vec![],
        );
        assert!(document.is_ok());
        let mut document = document.unwrap();
        document
            .update(Some(&reader::Content::UTF8("hello world!".to_string())))
            .unwrap();
        assert_eq!(document.to_string(), "hello world!");
        assert_eq!(document.get_deltas().len(), 1);
        assert_eq!(document.get_deltas()[0].operations.len(), 1);
        assert_eq!(
            document.get_deltas()[0].operations[0],
            Operation::Insert((11, "!".to_string()))
        );
    }

    #[test]
    fn test_empty() {
        let document = Document::new(None, vec![]);
        assert!(document.is_ok());
        let mut document = document.unwrap();
        document
            .update(Some(&reader::Content::UTF8("hello world!".to_string())))
            .unwrap();
        assert_eq!(document.to_string(), "hello world!");
        assert_eq!(document.get_deltas().len(), 1);
        assert_eq!(document.get_deltas()[0].operations.len(), 1);
        assert_eq!(
            document.get_deltas()[0].operations[0],
            Operation::Insert((0, "hello world!".to_string()))
        );
    }

    #[test]
    fn test_from_deltas() {
        let document = Document::new(
            None,
            vec![
                Delta {
                    timestamp_ms: 0,
                    operations: vec![Operation::Insert((0, "hello".to_string()))],
                },
                Delta {
                    timestamp_ms: 1,
                    operations: vec![Operation::Insert((5, " world".to_string()))],
                },
                Delta {
                    timestamp_ms: 2,
                    operations: vec![
                        Operation::Delete((3, 7)),
                        Operation::Insert((4, "!".to_string())),
                    ],
                },
            ],
        );
        assert!(document.is_ok());
        let document = document.unwrap();
        assert_eq!(document.to_string(), "held!");
    }

    #[test]
    fn test_complex_line() {
        let document = Document::new(None, vec![]);
        assert!(document.is_ok());
        let mut document = document.unwrap();

        document
            .update(Some(&reader::Content::UTF8("hello".to_string())))
            .unwrap();
        assert_eq!(document.to_string(), "hello");
        assert_eq!(document.get_deltas().len(), 1);
        assert_eq!(document.get_deltas()[0].operations.len(), 1);
        assert_eq!(
            document.get_deltas()[0].operations[0],
            Operation::Insert((0, "hello".to_string()))
        );

        document
            .update(Some(&reader::Content::UTF8("hello world".to_string())))
            .unwrap();
        assert_eq!(document.to_string(), "hello world");
        assert_eq!(document.get_deltas().len(), 2);
        assert_eq!(document.get_deltas()[1].operations.len(), 1);
        assert_eq!(
            document.get_deltas()[1].operations[0],
            Operation::Insert((5, " world".to_string()))
        );

        document
            .update(Some(&reader::Content::UTF8("held!".to_string())))
            .unwrap();
        assert_eq!(document.to_string(), "held!");
        assert_eq!(document.get_deltas().len(), 3);
        assert_eq!(document.get_deltas()[2].operations.len(), 2);
        assert_eq!(
            document.get_deltas()[2].operations[0],
            Operation::Delete((3, 7))
        );
        assert_eq!(
            document.get_deltas()[2].operations[1],
            Operation::Insert((4, "!".to_string())),
        );
    }

    #[test]
    fn test_multiline_add() {
        let document = Document::new(None, vec![]);
        assert!(document.is_ok());
        let mut document = document.unwrap();

        document
            .update(Some(&reader::Content::UTF8("first".to_string())))
            .unwrap();
        assert_eq!(document.to_string(), "first");
        assert_eq!(document.get_deltas().len(), 1);
        assert_eq!(document.get_deltas()[0].operations.len(), 1);
        assert_eq!(
            document.get_deltas()[0].operations[0],
            Operation::Insert((0, "first".to_string()))
        );

        document
            .update(Some(&reader::Content::UTF8("first\ntwo".to_string())))
            .unwrap();
        assert_eq!(document.to_string(), "first\ntwo");
        assert_eq!(document.get_deltas().len(), 2);
        assert_eq!(document.get_deltas()[1].operations.len(), 1);
        assert_eq!(
            document.get_deltas()[1].operations[0],
            Operation::Insert((5, "\ntwo".to_string()))
        );

        document
            .update(Some(&reader::Content::UTF8(
                "first line\nline two".to_string(),
            )))
            .unwrap();
        assert_eq!(document.to_string(), "first line\nline two");
        assert_eq!(document.get_deltas().len(), 3);
        assert_eq!(document.get_deltas()[2].operations.len(), 2);
        assert_eq!(
            document.get_deltas()[2].operations[0],
            Operation::Insert((5, " line".to_string()))
        );
        assert_eq!(
            document.get_deltas()[2].operations[1],
            Operation::Insert((11, "line ".to_string()))
        );
    }

    #[test]
    fn test_multiline_remove() {
        let document = Document::new(None, vec![]);
        assert!(document.is_ok());
        let mut document = document.unwrap();

        document
            .update(Some(&reader::Content::UTF8(
                "first line\nline two".to_string(),
            )))
            .unwrap();
        assert_eq!(document.to_string(), "first line\nline two");
        assert_eq!(document.get_deltas().len(), 1);
        assert_eq!(document.get_deltas()[0].operations.len(), 1);
        assert_eq!(
            document.get_deltas()[0].operations[0],
            Operation::Insert((0, "first line\nline two".to_string()))
        );

        document
            .update(Some(&reader::Content::UTF8("first\ntwo".to_string())))
            .unwrap();
        assert_eq!(document.to_string(), "first\ntwo");
        assert_eq!(document.get_deltas().len(), 2);
        assert_eq!(document.get_deltas()[1].operations.len(), 2);
        assert_eq!(
            document.get_deltas()[1].operations[0],
            Operation::Delete((5, 5))
        );
        assert_eq!(
            document.get_deltas()[1].operations[1],
            Operation::Delete((6, 5))
        );

        document
            .update(Some(&reader::Content::UTF8("first".to_string())))
            .unwrap();
        assert_eq!(document.to_string(), "first");
        assert_eq!(document.get_deltas().len(), 3);
        assert_eq!(document.get_deltas()[2].operations.len(), 1);
        assert_eq!(
            document.get_deltas()[2].operations[0],
            Operation::Delete((5, 4))
        );

        document.update(None).unwrap();
        assert_eq!(document.to_string(), "");
        assert_eq!(document.get_deltas().len(), 4);
        assert_eq!(document.get_deltas()[3].operations.len(), 1);
        assert_eq!(
            document.get_deltas()[3].operations[0],
            Operation::Delete((0, 5))
        );
    }

    #[test]
    fn test_binary_to_text() {
        let latest = reader::Content::Binary;
        let current = reader::Content::UTF8("test".to_string());
        let mut document = Document::new(Some(&latest), vec![]).unwrap();
        let new_deltas = document.update(Some(&current)).unwrap();
        assert!(new_deltas.is_some());
        assert_eq!(document.to_string(), "test");
    }

    #[test]
    fn test_binary_to_binary() {
        let latest = reader::Content::Binary;
        let current = reader::Content::Binary;
        let mut document = Document::new(Some(&latest), vec![]).unwrap();
        let new_deltas = document.update(Some(&current)).unwrap();
        assert!(new_deltas.is_some());
        assert_eq!(document.to_string(), "");
    }

    #[test]
    fn test_text_to_binary() {
        let latest = reader::Content::UTF8("text".to_string());
        let current = reader::Content::Binary;
        let mut document = Document::new(Some(&latest), vec![]).unwrap();
        let new_deltas = document.update(Some(&current)).unwrap();
        assert!(new_deltas.is_some());
        assert_eq!(document.to_string(), "");
    }

    #[test]
    fn test_unicode() {
        let latest = reader::Content::UTF8("🌚".to_string());
        let current = reader::Content::UTF8("🌝".to_string());
        let mut document = Document::new(Some(&latest), vec![]).unwrap();
        document.update(Some(&current)).unwrap();
        assert_eq!(document.to_string(), "🌝");
    }
}
