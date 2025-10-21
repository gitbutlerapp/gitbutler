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
