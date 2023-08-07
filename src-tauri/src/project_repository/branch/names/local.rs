use std::{fmt, str::FromStr};

use serde::{Deserialize, Serialize};

use super::{error::Error, RemoteName};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize, Serialize)]
pub struct Name {
    // contains name of the branch, e.x. "master" or "main"
    branch: String,
    // contains name of the remote branch, if the local branch is tracking a remote branch
    remote: Option<RemoteName>,
}

impl Name {
    pub fn branch(&self) -> &str {
        &self.branch
    }

    pub fn remote(&self) -> Option<&RemoteName> {
        self.remote.as_ref()
    }
}

impl fmt::Display for Name {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "refs/heads/{}", self.branch)
    }
}

impl FromStr for Name {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if !value.starts_with("refs/heads/") {
            return Err(Error::NotLocal(value.to_string()));
        }

        if let Some(branch) = value.strip_prefix("refs/heads/") {
            Ok(Self {
                branch: branch.to_string(),
                remote: None,
            })
        } else {
            Err(Error::InvalidName(value.to_string()))
        }
    }
}

impl TryFrom<&git2::Branch<'_>> for Name {
    type Error = Error;

    fn try_from(value: &git2::Branch<'_>) -> std::result::Result<Self, Self::Error> {
        let branch = String::from_utf8(value.name_bytes()?.to_vec()).map_err(Error::Utf8Error)?;
        if value.get().is_remote() {
            Err(Error::NotLocal(branch))
        } else {
            match value.upstream() {
                Ok(upstream) => Ok(Self {
                    branch,
                    remote: Some(RemoteName::try_from(&upstream)?),
                }),
                Err(error) => {
                    if error.code() == git2::ErrorCode::NotFound {
                        Ok(Self {
                            branch,
                            remote: None,
                        })
                    } else {
                        Err(error.into())
                    }
                }
            }
        }
    }
}
