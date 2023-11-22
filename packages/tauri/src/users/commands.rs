use tauri::{AppHandle, Manager};
use tracing::instrument;

use crate::{assets, error::Error, sentry};

use super::{
    controller::{self, Controller, GetError},
    User,
};

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

#[tauri::command(async)]
#[instrument(skip(handle))]
pub async fn set_current_project(
    handle: tauri::AppHandle,
    project_id: Option<&str>,
) -> Result<(), Error> {
    let app = handle.state::<Controller>();
    match app.get_user()? {
        Some(user) => {
            let mut user = user;
            match project_id {
                Some(project_id) => {
                    user.current_project = Some(project_id.to_string());
                }
                None => {
                    user.current_project = None;
                }
            }
            app.set_user(&user)?;
            if let Some(win) = handle.get_window("main") {
                let menu_handle = win.menu_handle();
                _ = menu_handle
                    .get_item("projectsettings")
                    .set_enabled(project_id.is_some());
            }

            Ok(())
        }
        None => Err({
            tracing::error!("failed to get user");
            Error::Unknown
        }),
    }
}

#[tauri::command(async)]
#[instrument(skip(handle))]
pub async fn get_current_project(handle: tauri::AppHandle) -> Result<Option<String>, Error> {
    let app = handle.state::<Controller>();
    match app.get_user()? {
        Some(user) => Ok(user.current_project),
        None => Err({
            tracing::error!("failed to get user");
            Error::Unknown
        }),
    }
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
