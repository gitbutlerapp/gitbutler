use crate::error::Error;
use anyhow::Result;
use but_api_macros::api_cmd;
use gitbutler_user::User;
use serde::{Deserialize, Serialize};
use tracing::instrument;

#[derive(Debug, Deserialize, Serialize)]
pub struct UserWithSecretsSensitive {
    pub id: u64,
    pub name: Option<String>,
    pub login: Option<String>,
    pub email: Option<String>,
    pub picture: String,
    pub locale: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub access_token: String,
    pub role: Option<String>,
    pub github_access_token: Option<String>,
    pub github_username: Option<String>,
}

impl TryFrom<User> for UserWithSecretsSensitive {
    type Error = anyhow::Error;

    fn try_from(value: User) -> Result<Self, Self::Error> {
        let access_token = value.access_token()?;
        let github_access_token = value.github_access_token()?;
        let User {
            id,
            name,
            login,
            email,
            picture,
            locale,
            created_at,
            updated_at,
            role,
            github_username,
            ..
        } = value;
        Ok(UserWithSecretsSensitive {
            id,
            name,
            login,
            email,
            picture,
            locale,
            created_at,
            updated_at,
            access_token: access_token.0,
            role,
            github_access_token: github_access_token.map(|s| s.0),
            github_username,
        })
    }
}

#[api_cmd]
#[cfg_attr(feature = "tauri", tauri::command(async))]
#[instrument(err(Debug))]
pub fn get_user() -> Result<Option<UserWithSecretsSensitive>, Error> {
    match gitbutler_user::get_user()? {
        Some(user) => {
            if let Err(err) = user.access_token() {
                gitbutler_user::delete_user()?;
                return Err(err.context("Please login to GitButler again").into());
            }
            Ok(Some(user.try_into()?))
        }
        None => Ok(None),
    }
}

#[api_cmd]
#[cfg_attr(feature = "tauri", tauri::command(async))]
#[instrument(err(Debug))]
pub fn set_user(user: User) -> Result<User, Error> {
    gitbutler_user::set_user(&user)?;
    Ok(user)
}

#[api_cmd]
#[cfg_attr(feature = "tauri", tauri::command(async))]
#[instrument(err(Debug))]
pub fn delete_user() -> Result<(), Error> {
    gitbutler_user::delete_user()?;
    Ok(())
}
