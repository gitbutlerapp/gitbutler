use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, PartialEq, Debug, Clone)]
#[serde(tag = "type", rename_all = "lowercase")]
/// Supported git forge types
pub enum ForgeType {
    GitHub,
    GitLab,
    Bitbucket,
    Azure,
}
