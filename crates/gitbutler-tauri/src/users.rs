use but_api::{commands::users, App, NoParams};
use gitbutler_user::User;
use tauri::State;
use tracing::instrument;

use but_api::error::Error;

#[tauri::command(async)]
#[instrument(skip(app), err(Debug))]
pub fn get_user(app: State<App>) -> Result<Option<users::UserWithSecrets>, Error> {
    users::get_user(&app, NoParams {})
}

#[tauri::command(async)]
#[instrument(skip(app), err(Debug))]
pub fn set_user(app: State<App>, user: User) -> Result<User, Error> {
    users::set_user(&app, users::SetUserParams { user })
}

#[tauri::command(async)]
#[instrument(skip(app), err(Debug))]
pub fn delete_user(app: State<App>) -> Result<(), Error> {
    users::delete_user(&app, NoParams {})
}
