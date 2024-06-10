use crate::types::Sensitive;
use serde::{Deserialize, Serialize};

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
    pub access_token: Sensitive<String>,
    pub role: Option<String>,
    pub github_access_token: Option<Sensitive<String>>,
    #[serde(default)]
    pub github_username: Option<String>,
}
