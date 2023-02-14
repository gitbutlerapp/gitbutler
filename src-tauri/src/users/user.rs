use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct User {
    pub id: String,
    pub name: String,
    pub email: String,
    pub access_token: String,
}

impl AsRef<User> for User {
    fn as_ref(&self) -> &User {
        self
    }
}

impl User {}
