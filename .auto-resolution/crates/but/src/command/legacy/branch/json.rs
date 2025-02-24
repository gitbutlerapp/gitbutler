use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct BranchNewOutput {
    pub branch: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub anchor: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BranchListOutput {
    pub applied_stacks: Vec<StackOutput>,
    pub branches: Vec<BranchOutput>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub more_branches: Option<usize>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StackOutput {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub heads: Vec<BranchHeadOutput>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BranchHeadOutput {
    pub name: String,
    pub reviews: Vec<ReviewOutput>,
    /// Last commit timestamp in milliseconds since epoch
    pub last_commit_at: u128,
    /// Number of commits ahead of the base branch
    pub commits_ahead: Option<usize>,
    pub last_author: AuthorOutput,
    /// Whether the branch merges cleanly into upstream
    #[serde(skip_serializing_if = "Option::is_none")]
    pub merges_cleanly: Option<bool>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BranchOutput {
    pub name: String,
    pub reviews: Vec<ReviewOutput>,
    pub has_local: bool,
    /// Last commit timestamp in milliseconds since epoch
    pub last_commit_at: u128,
    /// Number of commits ahead of the base branch
    pub commits_ahead: Option<usize>,
    pub last_author: AuthorOutput,
    /// Whether the branch merges cleanly into upstream
    #[serde(skip_serializing_if = "Option::is_none")]
    pub merges_cleanly: Option<bool>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorOutput {
    pub name: Option<String>,
    pub email: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReviewOutput {
    pub number: u64,
    pub url: String,
}
