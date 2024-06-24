use crate::types::Sensitive;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::cell::RefCell;

#[derive(Debug, Deserialize, Serialize, Clone, Default)]
pub struct User {
    pub id: u64,
    pub name: Option<String>,
    pub given_name: Option<String>,
    pub family_name: Option<String>,
    pub email: String,
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
        match self.access_token.borrow().as_ref() {
            Some(token) => Ok(token.clone()),
            None => {
                let err_msg = "BUG: access token for user must have been stored - delete user.json and login again to fix";
                let secret =
                    crate::secret::retrieve(Self::ACCESS_TOKEN_HANDLE)?.context(err_msg)?;
                *self.access_token.borrow_mut() = Some(secret.clone());
                Ok(secret)
            }
        }
    }

    /// Obtain the GitHub access token, if it is stored either on this instance or in the secrets store.
    ///
    /// Note that if retrieved from the secrets store, it will be cached on instance.
    pub fn github_access_token(&self) -> Result<Option<Sensitive<String>>> {
        match self.github_access_token.borrow().as_ref() {
            Some(token) => Ok(Some(token.clone())),
            None => {
                let secret = crate::secret::retrieve(Self::GITHUB_ACCESS_TOKEN_HANDLE)?;
                self.github_access_token.borrow_mut().clone_from(&secret);
                Ok(secret)
            }
        }
    }
}
