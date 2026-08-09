use anyhow::{Context, Result};
use but_api_macros::but_api;
use gitbutler_user::User;
use tracing::instrument;

mod json {
    use gitbutler_user::User;
    use serde::Serialize;

    #[derive(Debug, Serialize)]
    pub struct UserWithSecretsSensitive {
        pub id: u64,
        pub name: Option<String>,
        pub login: Option<String>,
        pub email: Option<String>,
        pub picture: String,
        pub locale: Option<String>,
        pub created_at: String,
        pub updated_at: String,
        /// GitButler access token.
        ///
        /// SECURITY: This is a secret credential. Do **not** log it, include it in telemetry,
        /// or expose it outside of a trusted/local API boundary. This struct is intended to be
        /// serialized only for trusted clients that need to perform authenticated GitButler
        /// API requests on behalf of the user.
        pub access_token: String,
        pub role: Option<String>,
        /// GitHub OAuth access token.
        ///
        /// SECURITY: This is a secret credential. Do **not** log it, include it in telemetry,
        /// or expose it outside of a trusted/local API boundary. This struct is intended to be
        /// serialized only for trusted clients that need to perform authenticated GitHub
        /// operations on behalf of the user.
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
}

/// The signed-in account, without any credential.
///
/// `json::UserWithSecretsSensitive` carries the GitButler and GitHub access tokens
/// because desktop calls the GitButler API from its frontend. A client that does not
/// should not be handed long-lived credentials to display a name and a picture.
#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct UserProfile {
    pub id: u64,
    pub name: Option<String>,
    pub login: Option<String>,
    pub email: Option<String>,
    pub picture: String,
    pub github_username: Option<String>,
}
but_schemars::register_sdk_type!(UserProfile);

impl From<User> for UserProfile {
    fn from(user: User) -> Self {
        UserProfile {
            id: user.id,
            name: user.name,
            login: user.login,
            email: user.email,
            picture: user.picture,
            github_username: user.github_username,
        }
    }
}

/// The signed-in account, or `None`. Credentials stay in this process.
#[but_api(napi)]
#[instrument(err(Debug))]
pub fn get_user_profile_local() -> Result<Option<UserProfile>> {
    Ok(get_user()?.map(Into::into))
}

/// Change the profile on gitbutler.com and keep the stored account in step.
///
/// The API call alone would leave the local copy stale, so the name shown next to the
/// picture would still be the old one until the next sign-in.
#[but_api(napi)]
#[instrument(skip(params), err(Debug))]
pub fn update_profile_and_persist(
    params: gitbutler_user::api::UpdateUserParams,
) -> Result<UserProfile> {
    let value = gitbutler_user::api::update_user_profile(params)?;
    let updated: User = serde_json::from_value(value)?;

    // Carry the changed fields onto the stored user rather than replacing it: the
    // credentials live behind private fields, and the API response has none to put back.
    let Some(mut stored) = gitbutler_user::get_user()? else {
        return Ok(updated.into());
    };
    stored.name = updated.name;
    stored.email = updated.email;
    stored.picture = updated.picture;
    gitbutler_user::set_user(&stored)?;
    Ok(stored.into())
}

/// Complete a login and persist the account, so the token never leaves this process.
#[but_api(napi)]
#[instrument(skip(token), err(Debug))]
pub fn login_and_persist(token: String) -> Result<UserProfile> {
    let value = gitbutler_user::api::fetch_user_by_token(&token)?;
    let user: User = serde_json::from_value(value)?;
    // A missing token deserializes to `None` rather than failing, which would store an
    // account that looks signed in and fails every later call instead of this one.
    user.access_token()
        .context("the login response carried no access token")?;
    gitbutler_user::set_user(&user)?;
    Ok(user.into())
}

#[but_api(try_from = json::UserWithSecretsSensitive)]
#[instrument(err(Debug))]
pub fn get_user() -> Result<Option<User>> {
    match gitbutler_user::get_user()? {
        Some(user) => {
            if let Err(err) = user.access_token() {
                gitbutler_user::delete_user()?;
                return Err(err.context("Please login to GitButler again"));
            }
            Ok(Some(user))
        }
        None => Ok(None),
    }
}

#[but_api]
#[instrument(err(Debug))]
pub fn set_user(user: User) -> Result<()> {
    gitbutler_user::set_user(&user)
}

#[but_api(napi)]
#[instrument(err(Debug))]
pub fn delete_user() -> Result<()> {
    gitbutler_user::delete_user()
}

#[but_api(napi)]
#[instrument(err(Debug))]
pub fn get_login_token() -> Result<gitbutler_user::api::LoginToken> {
    gitbutler_user::api::fetch_login_token()
}

#[but_api]
#[instrument(skip(token), err(Debug))]
pub fn login_with_token(token: String) -> Result<serde_json::Value> {
    gitbutler_user::api::fetch_user_by_token(&token)
}

#[but_api]
#[instrument(err(Debug))]
pub fn get_user_profile() -> Result<serde_json::Value> {
    gitbutler_user::api::fetch_user_profile()
}

#[but_api]
#[instrument(skip(params), err(Debug))]
pub fn update_user_profile(
    params: gitbutler_user::api::UpdateUserParams,
) -> Result<serde_json::Value> {
    gitbutler_user::api::update_user_profile(params)
}
