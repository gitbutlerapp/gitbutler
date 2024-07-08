pub mod commands {
    use gitbutler_branch::assets;
    use gitbutler_user::{controller::Controller, User};
    use serde::{Deserialize, Serialize};
    use tauri::{AppHandle, Manager};
    use tracing::instrument;

    use crate::error::Error;

    #[tauri::command(async)]
    #[instrument(skip(handle), err(Debug))]
    pub async fn get_user(handle: AppHandle) -> Result<Option<UserWithSecrets>, Error> {
        let app = handle.state::<Controller>();
        let proxy = handle.state::<assets::Proxy>();

        match app.get_user()? {
            Some(user) => {
                if let Err(err) = user.access_token() {
                    app.delete_user()?;
                    return Err(err.context("Please login to GitButler again").into());
                }
                Ok(Some(proxy.proxy_user(user).await.try_into()?))
            }
            None => Ok(None),
        }
    }

    #[tauri::command(async)]
    #[instrument(skip(handle), err(Debug))]
    pub async fn set_user(handle: AppHandle, user: User) -> Result<User, Error> {
        let app = handle.state::<Controller>();
        let proxy = handle.state::<assets::Proxy>();

        app.set_user(&user)?;

        Ok(proxy.proxy_user(user).await)
    }

    #[tauri::command(async)]
    #[instrument(skip(handle), err(Debug))]
    pub async fn delete_user(handle: AppHandle) -> Result<(), Error> {
        let app = handle.state::<Controller>();

        app.delete_user()?;

        Ok(())
    }

    #[derive(Debug, Deserialize, Serialize)]
    pub struct UserWithSecrets {
        id: u64,
        name: Option<String>,
        given_name: Option<String>,
        family_name: Option<String>,
        email: String,
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
                given_name,
                family_name,
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
                given_name,
                family_name,
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
