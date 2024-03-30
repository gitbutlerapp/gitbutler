use serde::{Deserialize, Serialize};

use super::operations;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Delta {
    pub operations: Vec<operations::Operation>,
    pub timestamp_ms: u128,
}
