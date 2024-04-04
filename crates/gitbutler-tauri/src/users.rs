pub mod commands {
    use gitbutler_core::{
        assets,
        users::{controller::Controller, User},
    };
    use tauri::{AppHandle, Manager};
    use tracing::instrument;

    use crate::error::Error;
    use crate::sentry;

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

    #[tauri::command(async)]
    #[instrument(skip(handle))]
    pub async fn set_user(handle: AppHandle, user: User) -> Result<User, Error> {
        let app = handle.state::<Controller>();
        let proxy = handle.state::<assets::Proxy>();

        app.set_user(&user)?;

        sentry::configure_scope(Some(&user));

        Ok(proxy.proxy_user(user).await)
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
