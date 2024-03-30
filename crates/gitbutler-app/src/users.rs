pub mod commands {
    use gitbutler::{
        assets,
        users::{
            controller::{self, Controller, GetError},
            User,
        },
    };
    use tauri::{AppHandle, Manager};
    use tracing::instrument;

    use crate::{error::Error, sentry};

    impl From<GetError> for Error {
        fn from(value: GetError) -> Self {
            match value {
                GetError::Other(error) => {
                    tracing::error!(?error, "failed to get user");
                    Error::Unknown
                }
            }
        }
    }

    #[tauri::command(async)]
    #[instrument(skip(handle))]
    pub async fn get_user(handle: AppHandle) -> Result<Option<User>, Error> {
        let app = handle.state::<Controller>();
        let proxy = handle.state::<assets::Proxy>();

        match app.get_user()? {
            Some(user) => Ok(Some(proxy.proxy_user(user).await)),
            None => Ok(None),
        }
    }

    impl From<controller::SetError> for Error {
        fn from(value: controller::SetError) -> Self {
            match value {
                controller::SetError::Other(error) => {
                    tracing::error!(?error, "failed to set user");
                    Error::Unknown
                }
            }
        }
    }

    #[tauri::command(async)]
    #[instrument(skip(handle))]
    pub async fn set_user(handle: AppHandle, user: User) -> Result<User, Error> {
        let app = handle.state::<Controller>();
        let proxy = handle.state::<assets::Proxy>();

        app.set_user(&user)?;

        sentry::configure_scope(Some(&user));

        Ok(proxy.proxy_user(user).await)
    }

    impl From<controller::DeleteError> for Error {
        fn from(value: controller::DeleteError) -> Self {
            match value {
                controller::DeleteError::Other(error) => {
                    tracing::error!(?error, "failed to delete user");
                    Error::Unknown
                }
            }
        }
    }

    #[tauri::command(async)]
    #[instrument(skip(handle))]
    pub async fn delete_user(handle: AppHandle) -> Result<(), Error> {
        let app = handle.state::<Controller>();

        app.delete_user()?;

        sentry::configure_scope(None);

        Ok(())
    }
}
