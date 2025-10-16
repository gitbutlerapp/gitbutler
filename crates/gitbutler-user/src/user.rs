use std::cell::RefCell;

use anyhow::{Context, Result};
use but_secret::{secret, Sensitive};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct User {
    pub id: u64,
    pub name: Option<String>,
    pub login: Option<String>,
    pub email: Option<String>,
    pub picture: String,
    pub locale: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    /// The presence of a GitButler access token is required for a valid user, but it's optional
    /// as it's not actually stored anymore, but fetch on demand in a separate step as its
    /// storage location is the [secrets store](crate::secret).
    #[serde(skip_serializing)]
    pub(super) access_token: RefCell<Option<Sensitive<String>>>,
    pub role: Option<String>,
    /// The semantics here are the same as for `access_token`, but this token is truly optional.
    #[serde(skip_serializing)]
    pub(super) github_access_token: RefCell<Option<Sensitive<String>>>,
    #[serde(default)]
    pub github_username: Option<String>,
}

impl User {
    pub(super) const ACCESS_TOKEN_HANDLE: &'static str = "gitbutler_access_token";
    pub(super) const GITHUB_ACCESS_TOKEN_HANDLE: &'static str = "github_access_token";

    /// Return the access token of the user after fetching it from the secrets store.
    ///
    /// It's cached after the first retrieval.
    pub fn access_token(&self) -> Result<Sensitive<String>> {
        if let Some(token) = self.access_token.borrow().as_ref() {
            return Ok(token.clone());
        }
        let err_msg = "access token for user was deleted from keychain - login is now invalid";
        let secret = secret::retrieve(Self::ACCESS_TOKEN_HANDLE, secret::Namespace::BuildKind)?
            .context(err_msg)?;
        *self.access_token.borrow_mut() = Some(secret.clone());
        Ok(secret)
    }

    /// Obtain the GitHub access token, if it is stored either on this instance or in the secrets store.
    ///
    /// Note that if retrieved from the secrets store, it will be cached on instance.
    pub fn github_access_token(&self) -> Result<Option<Sensitive<String>>> {
        if let Some(token) = self.github_access_token.borrow().as_ref() {
            return Ok(Some(token.clone()));
        }
        let secret = secret::retrieve(
            Self::GITHUB_ACCESS_TOKEN_HANDLE,
            secret::Namespace::BuildKind,
        )?;
        self.github_access_token.borrow_mut().clone_from(&secret);
        Ok(secret)
    }
}
