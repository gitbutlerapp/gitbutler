#[derive(Debug, PartialEq, Clone)]
pub struct Target {
    pub name: String,
    pub remote: String,
    pub sha: git2::Oid,
}

#[derive(Debug, PartialEq, Clone)]
pub struct Branch {
    pub id: String,
    pub name: String,
    pub target: Target,
    pub applied: bool,
    pub upstream: String,
    pub created_timestamp_ms: u128,
    pub updated_timestamp_ms: u128,
}
