use anyhow::Result;
use gitbutler_user::User;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::RequestContext;

#[derive(Debug, Deserialize, Serialize)]
pub struct UserWithSecrets {
    id: u64,
    name: Option<String>,
    login: Option<String>,
    email: Option<String>,
    picture: String,
    locale: Option<String>,
    created_at: String,
    updated_at: String,
    access_token: String,
    role: Option<String>,
    github_access_token: Option<String>,
    github_username: Option<String>,
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

pub fn get_user(ctx: &RequestContext) -> Result<serde_json::Value> {
    match ctx.user_controller.get_user()? {
        Some(user) => {
            if let Err(err) = user.access_token() {
                ctx.user_controller.delete_user()?;
                return Err(err.context("Please login to GitButler again"));
            }
            let user_with_secrets: UserWithSecrets = user.try_into()?;
            Ok(serde_json::to_value(user_with_secrets)?)
        }
        None => Ok(json!(null)),
    }
}

pub fn set_user(ctx: &RequestContext, params: serde_json::Value) -> Result<serde_json::Value> {
    let user: User = serde_json::from_value(params["user"].clone())?;
    ctx.user_controller.set_user(&user)?;
    Ok(serde_json::to_value(user)?)
}

pub fn delete_user(ctx: &RequestContext, _params: serde_json::Value) -> Result<serde_json::Value> {
    ctx.user_controller.delete_user()?;
    Ok(json!({}))
}
