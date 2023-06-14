use super::operations;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Delta {
    pub operations: Vec<operations::Operation>,
    pub timestamp_ms: u128,
}

impl Delta {
    fn apply_offset(operation: operations::Operation, offset: i64) -> operations::Operation {
        match operation {
            operations::Operation::Insert((index, chunk)) => {
                operations::Operation::Insert(((index as i64 - offset) as usize, chunk))
            }
            operations::Operation::Delete((index, len)) => {
                operations::Operation::Delete(((index as i64 - offset) as usize, len))
            }
        }
    }

    // take parts of another delta that are covered by this delta and return the rest with adjusted indices
    // (taken, rest)
    pub fn take(&self, another: &Delta) -> (Option<Delta>, Option<Delta>) {
        let mut taken_ops = Vec::new();
        let mut rest_ops = Vec::new();
        let mut rest_offset = 0;
        let mut taken_offset = 0;
        for operation in &another.operations {
            let mut taken = false;
            for self_op in &self.operations {
                if self_op.overlaps(operation) {
                    taken = true;
                    break;
                }
            }

            if !taken {
                rest_ops.push(Self::apply_offset(operation.clone(), taken_offset));
                rest_offset += operation.len_diff();
            } else {
                taken_ops.push(Self::apply_offset(operation.clone(), rest_offset));
                taken_offset += operation.len_diff();
            }
        }

        let taken = if !taken_ops.is_empty() {
            Some(Delta {
                operations: taken_ops,
                timestamp_ms: another.timestamp_ms,
            })
        } else {
            None
        };

        let rest = if !rest_ops.is_empty() {
            Some(Delta {
                operations: rest_ops,
                timestamp_ms: another.timestamp_ms,
            })
        } else {
            None
        };

        (taken, rest)
    }
}

#[cfg(test)]
mod tests {
    use std::vec;

    use super::*;

    #[test]
    fn take_full() {
        let left = Delta {
            operations: vec![operations::Operation::Insert((0, "one".to_string()))],
            timestamp_ms: 0,
        };
        let right = Delta {
            operations: vec![operations::Operation::Insert((3, "two".to_string()))],
            timestamp_ms: 1,
        };

        let result = left.take(&right);

        assert_eq!(
            result.0,
            Some(Delta {
                operations: vec![operations::Operation::Insert((3, "two".to_string()))],
                timestamp_ms: 1,
            })
        );
        assert_eq!(result.1, None);
    }

    #[test]
    fn take_none() {
        let left = Delta {
            operations: vec![operations::Operation::Insert((0, "one".to_string()))],
            timestamp_ms: 0,
        };
        let right = Delta {
            operations: vec![operations::Operation::Insert((5, "two".to_string()))],
            timestamp_ms: 1,
        };

        let result = left.take(&right);

        assert_eq!(result.0, None);
        assert_eq!(
            result.1,
            Some(Delta {
                operations: vec![operations::Operation::Insert((5, "two".to_string()))],
                timestamp_ms: 1,
            })
        );
    }

    #[test]
    fn take_some() {
        let left = Delta {
            operations: vec![operations::Operation::Insert((0, "one".to_string()))],
            timestamp_ms: 0,
        };
        let right = Delta {
            operations: vec![
                operations::Operation::Insert((3, "two".to_string())),
                operations::Operation::Insert((7, "four".to_string())),
            ],
            timestamp_ms: 1,
        };

        let result = left.take(&right);

        assert_eq!(
            result.0,
            Some(Delta {
                operations: vec![operations::Operation::Insert((3, "two".to_string()))],
                timestamp_ms: 1,
            })
        );
        assert_eq!(
            result.1,
            Some(Delta {
                operations: vec![operations::Operation::Insert((4, "four".to_string()))],
                timestamp_ms: 1,
            })
        );
    }
}
