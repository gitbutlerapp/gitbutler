use std::{fmt, str::FromStr};

use serde::{Deserialize, Serialize};

use super::{error::Error, remote};

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Refname {
    // contains name of the branch, e.x. "master" or "main"
    branch: String,
    // contains name of the remote branch, if the local branch is tracking a remote branch
    remote: Option<remote::Refname>,
}

impl Refname {
    pub fn new(branch: &str, remote: Option<remote::Refname>) -> Self {
        Self {
            branch: branch.to_string(),
            remote,
        }
    }

    pub fn branch(&self) -> &str {
        &self.branch
    }

    pub fn remote(&self) -> Option<&remote::Refname> {
        self.remote.as_ref()
    }
}

impl Serialize for Refname {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'d> Deserialize<'d> for Refname {
    fn deserialize<D: serde::Deserializer<'d>>(deserializer: D) -> Result<Self, D::Error> {
        let name = String::deserialize(deserializer)?;
        name.as_str().parse().map_err(serde::de::Error::custom)
    }
}

impl fmt::Display for Refname {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "refs/heads/{}", self.branch)
    }
}

impl FromStr for Refname {
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

impl TryFrom<&git2::Branch<'_>> for Refname {
    type Error = Error;

    fn try_from(value: &git2::Branch<'_>) -> std::result::Result<Self, Self::Error> {
        let branch_name =
            String::from_utf8(value.get().name_bytes().to_vec()).map_err(Error::Utf8)?;
        if value.get().is_remote() {
            Err(Error::NotLocal(branch_name))
        } else {
            let branch: Self = branch_name.parse()?;
            match value.upstream() {
                Ok(upstream) => Ok(Self {
                    remote: Some(remote::Refname::try_from(&upstream)?),
                    ..branch
                }),
                Err(error) => match error.code() {
                    git2::ErrorCode::NotFound => Ok(Self {
                        remote: None,
                        ..branch
                    }),
                    _ => Err(error.into()),
                },
            }
        }
    }
}
