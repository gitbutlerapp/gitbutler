pub mod commands {
    use gitbutler_user::{Controller, User};
    use serde::{Deserialize, Serialize};
    use tauri::State;
    use tracing::instrument;

    use crate::error::Error;

    #[tauri::command(async)]
    #[instrument(skip(login), err(Debug))]
    pub fn get_user(login: State<'_, Controller>) -> Result<Option<UserWithSecrets>, Error> {
        match login.get_user()? {
            Some(user) => {
                if let Err(err) = user.access_token() {
                    login.delete_user()?;
                    return Err(err.context("Please login to GitButler again").into());
                }
                Ok(Some(user.try_into()?))
            }
            None => Ok(None),
        }
    }

    #[tauri::command(async)]
    #[instrument(skip(login), err(Debug))]
    pub fn set_user(login: State<'_, Controller>, user: User) -> Result<User, Error> {
        login.set_user(&user)?;
        Ok(user)
    }

    #[tauri::command(async)]
    #[instrument(skip(login), err(Debug))]
    pub fn delete_user(login: State<'_, Controller>) -> Result<(), Error> {
        login.delete_user()?;
        Ok(())
    }

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
}
