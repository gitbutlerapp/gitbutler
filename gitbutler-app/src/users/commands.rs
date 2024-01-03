use tauri::{AppHandle, Manager};
use tracing::instrument;

use crate::{assets, error::UserError, sentry};

use super::{
    controller::{self, Controller, GetError},
    User,
};

impl From<GetError> for UserError {
    fn from(value: GetError) -> Self {
        match value {
            GetError::Other(error) => {
                tracing::error!(?error, "failed to get user");
                UserError::Unknown
            }
        }
    }
}

#[tauri::command(async)]
#[instrument(skip(handle))]
pub async fn get_user(handle: AppHandle) -> Result<Option<User>, UserError> {
    let app = handle.state::<Controller>();
    let proxy = handle.state::<assets::Proxy>();

    match app.get_user()? {
        Some(user) => Ok(Some(proxy.proxy_user(user).await)),
        None => Ok(None),
    }
}

impl From<controller::SetError> for UserError {
    fn from(value: controller::SetError) -> Self {
        match value {
            controller::SetError::Other(error) => {
                tracing::error!(?error, "failed to set user");
                UserError::Unknown
            }
        }
    }
}

#[tauri::command(async)]
#[instrument(skip(handle))]
pub async fn set_user(handle: AppHandle, user: User) -> Result<User, UserError> {
    let app = handle.state::<Controller>();
    let proxy = handle.state::<assets::Proxy>();

    app.set_user(&user)?;

    sentry::configure_scope(Some(&user));

    Ok(proxy.proxy_user(user).await)
}

impl From<controller::DeleteError> for UserError {
    fn from(value: controller::DeleteError) -> Self {
        match value {
            controller::DeleteError::Other(error) => {
                tracing::error!(?error, "failed to delete user");
                UserError::Unknown
            }
        }
    }
}

#[tauri::command(async)]
#[instrument(skip(handle))]
pub async fn delete_user(handle: AppHandle) -> Result<(), UserError> {
    let app = handle.state::<Controller>();

    app.delete_user()?;

    sentry::configure_scope(None);

    Ok(())
}
