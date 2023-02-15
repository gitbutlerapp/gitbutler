use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct User {
    pub id: u64,
    pub name: String,
    pub email: String,
    pub picture: String,
    pub locale: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub access_token: String,
}
