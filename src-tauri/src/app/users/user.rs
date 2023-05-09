use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
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

impl Into<sentry::User> for User {
    fn into(self) -> sentry::User {
        sentry::User {
            id: Some(self.id.to_string()),
            username: Some(self.name),
            email: Some(self.email),
            ..Default::default()
        }
    }
}
