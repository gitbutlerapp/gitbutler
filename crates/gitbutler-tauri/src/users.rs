use but_api::commands::users;
use gitbutler_user::User;

use but_api::error::Error;

#[tauri::command(async)]
pub fn get_user() -> Result<Option<users::UserWithSecrets>, Error> {
    users::get_user()
}

#[tauri::command(async)]
pub fn set_user(user: User) -> Result<User, Error> {
    users::set_user(user)
}

#[tauri::command(async)]
pub fn delete_user() -> Result<(), Error> {
    users::delete_user()
}
