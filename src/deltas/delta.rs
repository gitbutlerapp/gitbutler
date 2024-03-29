use super::operations;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Delta {
    pub operations: Vec<operations::Operation>,
    pub timestamp_ms: u128,
}
