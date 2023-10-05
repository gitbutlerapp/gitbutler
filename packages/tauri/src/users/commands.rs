use tauri::{AppHandle, Manager};

use tracing::instrument;

#[tauri::command(async)]
#[instrument(skip(handle))]
pub async fn get_user(handle: tauri::AppHandle) -> Result<Option<users::User>, Error> {
    let app = handle.state::<app::App>();
    let proxy = handle.state::<assets::Proxy>();

    match app.get_user().context("failed to get user")? {
        Some(user) => {
            let remote_picture = url::Url::parse(&user.picture).context("invalid picture url")?;
            let local_picture = match proxy.proxy(&remote_picture).await {
                Ok(picture) => picture,
                Err(e) => {
                    tracing::error!("{:#}", e);
                    remote_picture
                }
            };

            let user = users::User {
                picture: local_picture.to_string(),
                ..user
            };

            Ok(Some(user))
        }
        None => Ok(None),
    }
}

#[tauri::command(async)]
#[instrument(skip(handle))]
pub async fn set_user(handle: tauri::AppHandle, user: users::User) -> Result<(), Error> {
    let app = handle.state::<app::App>();

    app.set_user(&user).context("failed to set user")?;

    sentry::configure_scope(|scope| scope.set_user(Some(user.clone().into())));

    Ok(())
}

#[tauri::command(async)]
#[instrument(skip(handle))]
pub async fn delete_user(handle: tauri::AppHandle) -> Result<(), Error> {
    let app = handle.state::<app::App>();

    app.delete_user().context("failed to delete user")?;

    sentry::configure_scope(|scope| scope.set_user(None));

    Ok(())
}


