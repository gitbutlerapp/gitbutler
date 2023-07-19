use std::fmt;

use serde::{Deserialize, Serialize};

use super::{error::Error, remote};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Name {
    // contains name of the branch, e.x. "master" or "main"
    branch: String,
    // contains name of the remote branch, if the local branch is tracking a remote branch
    remote: Option<remote::Name>,
}

impl Name {
    pub fn branch(&self) -> &str {
        &self.branch
    }
}

impl Serialize for Name {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.branch)
    }
}

impl<'d> Deserialize<'d> for Name {
    fn deserialize<D: serde::Deserializer<'d>>(deserializer: D) -> Result<Self, D::Error> {
        let name = String::deserialize(deserializer)?;
        Self::try_from(name.as_str()).map_err(serde::de::Error::custom)
    }
}

impl fmt::Display for Name {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "refs/heads/{}", self.branch)
    }
}

impl TryFrom<&str> for Name {
    type Error = Error;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
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
                    remote: Some(remote::Name::try_from(&upstream)?),
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
