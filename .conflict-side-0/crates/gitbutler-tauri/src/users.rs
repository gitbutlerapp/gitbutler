use but_api::{commands::users, IpcContext, NoParams};
use gitbutler_user::User;
use tauri::State;
use tracing::instrument;

use but_api::error::Error;

#[tauri::command(async)]
#[instrument(skip(ipc_ctx), err(Debug))]
pub fn get_user(ipc_ctx: State<IpcContext>) -> Result<Option<users::UserWithSecrets>, Error> {
    users::get_user(&ipc_ctx, NoParams {})
}

#[tauri::command(async)]
#[instrument(skip(ipc_ctx), err(Debug))]
pub fn set_user(ipc_ctx: State<IpcContext>, user: User) -> Result<User, Error> {
    users::set_user(&ipc_ctx, users::SetUserParams { user })
}

#[tauri::command(async)]
#[instrument(skip(ipc_ctx), err(Debug))]
pub fn delete_user(ipc_ctx: State<IpcContext>) -> Result<(), Error> {
    users::delete_user(&ipc_ctx, NoParams {})
}
