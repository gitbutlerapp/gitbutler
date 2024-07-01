use serde::Serialize;

#[derive(Debug, Serialize, PartialEq, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CommitMetrics {
    pub name: String,
    pub value: usize,
    pub commit_ids: Vec<String>,
}

