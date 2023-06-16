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
    pub fn new(value: Option<&str>, deltas: Vec<delta::Delta>) -> Result<Document> {
        let mut doc = value.unwrap_or("").chars().collect::<Vec<char>>();
        apply_deltas(&mut doc, &deltas)?;
        Ok(Document { doc, deltas })
    }

    pub fn update(&mut self, value: &str) -> Result<Option<delta::Delta>> {
        let operations = operations::get_delta_operations(&self.to_string(), value);
        if operations.is_empty() {
            return Ok(None);
        }
        let delta = delta::Delta {
            operations,
            timestamp_ms: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_millis(),
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
        let document = Document::new(Some("hello world"), vec![]);
        assert!(document.is_ok());
        let document = document.unwrap();
        assert_eq!(document.to_string(), "hello world");
        assert_eq!(document.get_deltas().len(), 0);
    }

    #[test]
    fn test_update() {
        let document = Document::new(Some("hello world"), vec![]);
        assert!(document.is_ok());
        let mut document = document.unwrap();
        document.update("hello world!").unwrap();
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
        document.update("hello world!").unwrap();
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

        document.update("hello").unwrap();
        assert_eq!(document.to_string(), "hello");
        assert_eq!(document.get_deltas().len(), 1);
        assert_eq!(document.get_deltas()[0].operations.len(), 1);
        assert_eq!(
            document.get_deltas()[0].operations[0],
            Operation::Insert((0, "hello".to_string()))
        );

        document.update("hello world").unwrap();
        assert_eq!(document.to_string(), "hello world");
        assert_eq!(document.get_deltas().len(), 2);
        assert_eq!(document.get_deltas()[1].operations.len(), 1);
        assert_eq!(
            document.get_deltas()[1].operations[0],
            Operation::Insert((5, " world".to_string()))
        );

        document.update("held!").unwrap();
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

        document.update("first").unwrap();
        assert_eq!(document.to_string(), "first");
        assert_eq!(document.get_deltas().len(), 1);
        assert_eq!(document.get_deltas()[0].operations.len(), 1);
        assert_eq!(
            document.get_deltas()[0].operations[0],
            Operation::Insert((0, "first".to_string()))
        );

        document.update("first\ntwo").unwrap();
        assert_eq!(document.to_string(), "first\ntwo");
        assert_eq!(document.get_deltas().len(), 2);
        assert_eq!(document.get_deltas()[1].operations.len(), 1);
        assert_eq!(
            document.get_deltas()[1].operations[0],
            Operation::Insert((5, "\ntwo".to_string()))
        );

        document.update("first line\nline two").unwrap();
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

        document.update("first line\nline two").unwrap();
        assert_eq!(document.to_string(), "first line\nline two");
        assert_eq!(document.get_deltas().len(), 1);
        assert_eq!(document.get_deltas()[0].operations.len(), 1);
        assert_eq!(
            document.get_deltas()[0].operations[0],
            Operation::Insert((0, "first line\nline two".to_string()))
        );

        document.update("first\ntwo").unwrap();
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

        document.update("first").unwrap();
        assert_eq!(document.to_string(), "first");
        assert_eq!(document.get_deltas().len(), 3);
        assert_eq!(document.get_deltas()[2].operations.len(), 1);
        assert_eq!(
            document.get_deltas()[2].operations[0],
            Operation::Delete((5, 4))
        );

        document.update("").unwrap();
        assert_eq!(document.to_string(), "");
        assert_eq!(document.get_deltas().len(), 4);
        assert_eq!(document.get_deltas()[3].operations.len(), 1);
        assert_eq!(
            document.get_deltas()[3].operations[0],
            Operation::Delete((0, 5))
        );
    }

    #[test]
    fn test_unicode() {
        let latest = Some("🌚");
        let current = "🌝";
        let mut document = Document::new(latest, vec![]).unwrap();
        document.update(current).unwrap();
        assert_eq!(document.to_string(), "🌝");
    }
}
