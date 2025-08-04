mod controller;

mod storage;
use controller::Controller;

mod user;
pub use user::User;

pub fn get_user() -> anyhow::Result<Option<User>> {
    let controller = Controller::from_path(but_path::app_data_dir()?);
    controller.get_user()
}

/// Testing purpose only.
pub fn get_user_with_path<P: AsRef<std::path::Path>>(data_dir: P) -> anyhow::Result<Option<User>> {
    let controller = Controller::from_path(data_dir.as_ref());
    controller.get_user()
}

pub fn set_user(user: &User) -> anyhow::Result<()> {
    let controller = Controller::from_path(but_path::app_data_dir()?);
    controller.set_user(user)
}

/// Testing purpose only.
pub fn set_user_with_path<P: AsRef<std::path::Path>>(
    data_dir: P,
    user: &User,
) -> anyhow::Result<()> {
    let controller = Controller::from_path(data_dir.as_ref());
    controller.set_user(user)
}

pub fn delete_user() -> anyhow::Result<()> {
    let controller = Controller::from_path(but_path::app_data_dir()?);
    controller.delete_user()
}

/// Testing purpose only.
pub fn delete_user_with_path<P: AsRef<std::path::Path>>(data_dir: P) -> anyhow::Result<()> {
    let controller = Controller::from_path(data_dir.as_ref());
    controller.delete_user()
}
