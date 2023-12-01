use serde::{Deserialize, Serialize};

use crate::git;

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct User {
    pub id: u64,
    pub name: Option<String>,
    pub given_name: Option<String>,
    pub family_name: Option<String>,
    pub email: String,
    pub picture: String,
    pub locale: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub access_token: String,
    pub role: Option<String>,
    pub github_access_token: Option<String>,
    #[serde(default)]
    pub github_username: Option<String>,
}

impl TryFrom<User> for git::Signature<'_> {
    type Error = git::Error;

    fn try_from(value: User) -> Result<Self, Self::Error> {
        if let Some(name) = value.name {
            git::Signature::now(&name, &value.email)
        } else if let Some(name) = value.given_name {
            git::Signature::now(&name, &value.email)
        } else {
            git::Signature::now(&value.email, &value.email)
        }
    }
}
