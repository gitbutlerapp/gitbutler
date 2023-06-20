use std::cmp::Ordering;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use similar::{ChangeTag, TextDiff};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum Operation {
    // corresponds to YText.insert(index, chunk)
    Insert((usize, String)),
    // corresponds to YText.remove_range(index, len)
    Delete((usize, usize)),
}

impl Operation {
    pub fn apply(&self, text: &mut Vec<char>) -> Result<()> {
        match self {
            Operation::Insert((index, chunk)) => match index.cmp(&text.len()) {
                Ordering::Greater => Err(anyhow::anyhow!(
                    "Index out of bounds, {} > {}",
                    index,
                    text.len()
                )),
                Ordering::Equal => {
                    text.extend(chunk.chars());
                    Ok(())
                }
                Ordering::Less => {
                    text.splice(*index..*index, chunk.chars());
                    Ok(())
                }
            },
            Operation::Delete((index, len)) => {
                if *index > text.len() {
                    Err(anyhow::anyhow!(
                        "Index out of bounds, {} > {}",
                        index,
                        text.len()
                    ))
                } else if *index + *len > text.len() {
                    Err(anyhow::anyhow!(
                        "Index + length out of bounds, {} > {}",
                        index + len,
                        text.len()
                    ))
                } else {
                    text.splice(*index..(*index + *len), "".chars());
                    Ok(())
                }
            }
        }
    }
}

// merges touching operations of the same type in to one operation
// e.g. [Insert((0, "hello")), Insert((5, " world"))] -> [Insert((0, "hello world"))]
// e.g. [Delete((0, 5)), Delete((5, 5))] -> [Delete((0, 10))]
// e.g. [Insert((0, "hello")), Delete((0, 5))] -> [Insert((0, "hello")), Delete((0, 5))]
fn merge_touching(ops: &Vec<Operation>) -> Vec<Operation> {
    let mut merged = vec![];

    for op in ops {
        match (merged.last_mut(), op) {
            (Some(Operation::Insert((index, chunk))), Operation::Insert((index2, chunk2))) => {
                if *index + chunk.len() == *index2 {
                    chunk.push_str(chunk2);
                } else {
                    merged.push(op.clone());
                }
            }
            (Some(Operation::Delete((index, len))), Operation::Delete((index2, len2))) => {
                if *index == *index2 {
                    *len += len2;
                } else {
                    merged.push(op.clone());
                }
            }
            _ => merged.push(op.clone()),
        }
    }

    merged
}

pub fn get_delta_operations(initial_text: &str, final_text: &str) -> Vec<Operation> {
    if initial_text == final_text {
        return vec![];
    }

    let changeset = TextDiff::configure().diff_graphemes(initial_text, final_text);
    let mut deltas = vec![];

    let mut offset = 0;
    for change in changeset.iter_all_changes() {
        match change.tag() {
            ChangeTag::Delete => {
                deltas.push(Operation::Delete((
                    offset,
                    change.as_str().unwrap_or("").chars().count(),
                )));
            }
            ChangeTag::Insert => {
                let text = change.as_str().unwrap();
                deltas.push(Operation::Insert((offset, text.to_string())));
                offset = change.new_index().unwrap() + text.chars().count()
            }
            ChangeTag::Equal => {
                let text = change.as_str().unwrap();
                offset = change.new_index().unwrap() + text.chars().count()
            }
        }
    }

    merge_touching(&deltas)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_delta_operations_insert_end() {
        let initial_text = "hello";
        let final_text = "hello world!";
        let operations = get_delta_operations(initial_text, final_text);
        assert_eq!(operations.len(), 1);
        assert_eq!(operations[0], Operation::Insert((5, " world!".to_string())));
    }

    #[test]
    fn test_get_delta_operations_insert_middle() {
        let initial_text = "helloworld";
        let final_text = "hello, world";
        let operations = get_delta_operations(initial_text, final_text);
        assert_eq!(operations.len(), 1);
        assert_eq!(operations[0], Operation::Insert((5, ", ".to_string())));
    }

    #[test]
    fn test_get_delta_operations_insert_begin() {
        let initial_text = "world";
        let final_text = "hello world";
        let operations = get_delta_operations(initial_text, final_text);
        assert_eq!(operations.len(), 1);
        assert_eq!(operations[0], Operation::Insert((0, "hello ".to_string())));
    }

    #[test]
    fn test_get_delta_operations_delete_end() {
        let initial_text = "hello world!";
        let final_text = "hello";
        let operations = get_delta_operations(initial_text, final_text);
        assert_eq!(operations.len(), 1);
        assert_eq!(operations[0], Operation::Delete((5, 7)));
    }

    #[test]
    fn test_get_delta_operations_delete_middle() {
        let initial_text = "hello, world";
        let final_text = "helloworld";
        let operations = get_delta_operations(initial_text, final_text);
        assert_eq!(operations.len(), 1);
        assert_eq!(operations[0], Operation::Delete((5, 2)));
    }

    #[test]
    fn test_get_delta_operations_delete_begin() {
        let initial_text = "hello world";
        let final_text = "world";
        let operations = get_delta_operations(initial_text, final_text);
        assert_eq!(operations.len(), 1);
        assert_eq!(operations[0], Operation::Delete((0, 6)));
    }
}
