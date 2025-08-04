use anyhow::Result;
use gitbutler_user::User;
use serde::{Deserialize, Serialize};

use crate::{App, NoParams, error::Error};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SetUserParams {
    pub user: User,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct UserWithSecrets {
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

impl TryFrom<User> for UserWithSecrets {
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
        Ok(UserWithSecrets {
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

pub fn get_user(app: &App, _params: NoParams) -> Result<Option<UserWithSecrets>, Error> {
    match app.user_controller.get_user()? {
        Some(user) => {
            if let Err(err) = user.access_token() {
                app.user_controller.delete_user()?;
                return Err(err.context("Please login to GitButler again").into());
            }
            Ok(Some(user.try_into()?))
        }
        None => Ok(None),
    }
}

pub fn set_user(app: &App, params: SetUserParams) -> Result<User, Error> {
    app.user_controller.set_user(&params.user)?;
    Ok(params.user)
}

pub fn delete_user(app: &App, _params: NoParams) -> Result<(), Error> {
    app.user_controller.delete_user()?;
    Ok(())
}
