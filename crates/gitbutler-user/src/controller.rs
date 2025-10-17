use std::path::PathBuf;

use anyhow::{Context, Result};
use but_secret::secret;

use super::{User, storage::Storage};

/// TODO(ST): rename to `Login` - seems more akin to what it does
/// This type deals with user-related data which is only known if the user is logged in to GitButler.
///
/// ### Migrations: V1 -> V2
///
/// V2 is implied by not storing the `access_token` in plain text anymore, nor the GitHub token even if present.
/// It happens automatically on [Self::get_user()] and [Self::set_user()]
#[derive(Clone)]
pub(crate) struct Controller {
    storage: Storage,
}

impl Controller {
    pub fn from_path(path: impl Into<PathBuf>) -> Controller {
        Controller {
            storage: Storage::from_path(path),
        }
    }

    /// Return the current login, or `None` if there is none yet.
    pub(crate) fn get_user(&self) -> Result<Option<User>> {
        let user = self.storage.get().context("failed to get user")?;
        if let Some(user) = &user {
            write_without_secrets_if_secrets_present(&self.storage, user.clone())?;
        }
        Ok(user)
    }

    /// Note that secrets are never written in plain text, but we assure they are stored.
    pub(crate) fn set_user(&self, user: &User) -> Result<()> {
        if !write_without_secrets_if_secrets_present(&self.storage, user.clone())? {
            self.storage.set(user).context("failed to set user")
        } else {
            Ok(())
        }
    }

    pub(crate) fn delete_user(&self) -> Result<()> {
        self.storage.delete().context("failed to delete user")?;
        let namespace = secret::Namespace::BuildKind;
        secret::delete(User::ACCESS_TOKEN_HANDLE, namespace).ok();
        secret::delete(User::GITHUB_ACCESS_TOKEN_HANDLE, namespace).ok();
        Ok(())
    }
}

/// As `user` sports interior mutability right now, let's play it safe and work with fully owned items only.
fn write_without_secrets_if_secrets_present(storage: &Storage, user: User) -> Result<bool> {
    let mut needs_write = false;
    let namespace = secret::Namespace::BuildKind;
    if let Some(gb_token) = user.access_token.borrow_mut().take() {
        needs_write |= secret::persist(User::ACCESS_TOKEN_HANDLE, &gb_token, namespace).is_ok();
    }
    if let Some(gh_token) = user.github_access_token.borrow_mut().take() {
        needs_write |=
            secret::persist(User::GITHUB_ACCESS_TOKEN_HANDLE, &gh_token, namespace).is_ok();
    }
    if needs_write {
        storage.set(&user)?;
    }
    Ok(needs_write)
}
