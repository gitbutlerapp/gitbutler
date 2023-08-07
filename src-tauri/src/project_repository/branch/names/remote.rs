use std::{fmt, str::FromStr};

use serde::{Deserialize, Serialize};

use super::error::Error;

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct Name {
    // contains name of the remote, e.x. "origin" or "upstream"
    remote: String,
    // contains name of the branch, e.x. "master" or "main"
    branch: String,
}

impl Name {
    pub fn with_branch(&self, branch: &str) -> Self {
        Self {
            branch: branch.to_string(),
            remote: self.remote.clone(),
        }
    }

    pub fn branch(&self) -> &str {
        &self.branch
    }

    pub fn remote(&self) -> &str {
        &self.remote
    }
}

impl fmt::Display for Name {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "refs/remotes/{}/{}", self.remote, self.branch)
    }
}

impl Serialize for Name {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'d> Deserialize<'d> for Name {
    fn deserialize<D: serde::Deserializer<'d>>(deserializer: D) -> Result<Self, D::Error> {
        let name = String::deserialize(deserializer)?;
        name.as_str().parse().map_err(serde::de::Error::custom)
    }
}

impl FromStr for Name {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if !value.starts_with("refs/remotes/") {
            return Err(Error::NotRemote(value.to_string()));
        };

        let value = value.strip_prefix("refs/remotes/").unwrap();

        if let Some((remote, branch)) = value.split_once('/') {
            Ok(Self {
                remote: remote.to_string(),
                branch: branch.to_string(),
            })
        } else {
            Err(Error::InvalidName(value.to_string()))
        }
    }
}

impl TryFrom<&git2::Branch<'_>> for Name {
    type Error = Error;

    fn try_from(value: &git2::Branch<'_>) -> std::result::Result<Self, Self::Error> {
        let refname =
            String::from_utf8(value.get().name_bytes().to_vec()).map_err(Error::Utf8Error)?;

        if !value.get().is_remote() {
            return Err(Error::NotRemote(refname));
        }

        refname.as_str().parse()
    }
}
