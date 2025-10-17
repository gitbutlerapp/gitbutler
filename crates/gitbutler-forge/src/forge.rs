use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
#[serde(rename_all = "lowercase")]
/// Supported git forge types
pub enum ForgeName {
    GitHub,
    GitLab,
    Bitbucket,
    Azure,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ForgeRepoInfo {
    pub forge: ForgeName,
    pub owner: String,
    pub repo: String,
    pub protocol: String,
}

impl PartialEq for ForgeRepoInfo {
    fn eq(&self, other: &Self) -> bool {
        self.forge == other.forge && self.owner == other.owner && self.repo == other.repo
    }
}
