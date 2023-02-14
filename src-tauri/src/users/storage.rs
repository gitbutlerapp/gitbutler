use crate::storage;
use crate::users::user;
use anyhow::Result;
use serde_json::Value;
const USER_FILE: &str = "user.json";

pub struct Storage {
    storage: storage::Storage,
}

impl Storage {
    pub fn new(storage: storage::Storage) -> Self {
        Self { storage }
    }

    pub fn get_user(&self) -> Result<Option<user::User>> {
        match self.storage.read(USER_FILE) {
            Ok(content) => match content {
                Some(content) => {
                    match serde_json::from_str::<Value>(content.as_str()) {
                        Ok(v) => {
                            let u = user::User {
                                id: v["id"].to_string(),
                                name: v["name"].as_str().unwrap().to_string(),
                                email: v["email"].as_str().unwrap().to_string(),
                                access_token: v["access_token"].as_str().unwrap().to_string(),
                            };
                            return Ok(Some(u));
                        }
                        Err(e) => {
                            println!("Error: {:#?}", e);
                            return Err(anyhow::anyhow!("Error: {:?}", e));
                        }
                    };
                }
                None => return Ok(None),
            },
            Err(_) => return Ok(None),
        }
    }
}
