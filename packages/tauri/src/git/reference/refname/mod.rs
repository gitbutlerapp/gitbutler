mod error;
mod local;
mod remote;

use std::{fmt, str::FromStr};

use serde::{Deserialize, Serialize};

pub use error::Error;
pub use local::Refname as LocalRefname;
pub use remote::Refname as RemoteRefname;

use crate::git;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Refname {
    Remote(RemoteRefname),
    Local(LocalRefname),
}

impl From<&RemoteRefname> for Refname {
    fn from(value: &RemoteRefname) -> Self {
        Self::Remote(value.clone())
    }
}

impl From<RemoteRefname> for Refname {
    fn from(value: RemoteRefname) -> Self {
        Self::Remote(value)
    }
}

impl From<LocalRefname> for Refname {
    fn from(value: LocalRefname) -> Self {
        Self::Local(value)
    }
}

impl From<&LocalRefname> for Refname {
    fn from(value: &LocalRefname) -> Self {
        Self::Local(value.clone())
    }
}

impl Refname {
    pub fn branch(&self) -> &str {
        match self {
            Self::Remote(remote) => remote.branch(),
            Self::Local(local) => local.branch(),
        }
    }
}

impl FromStr for Refname {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if value.starts_with("refs") {
            if value.starts_with("refs/remotes/") {
                Ok(Self::Remote(value.parse()?))
            } else if value.starts_with("refs/heads/") {
                Ok(Self::Local(value.parse()?))
            } else {
                Err(Error::InvalidName(value.to_string()))
            }
        } else {
            Ok(Self::Local(value.parse()?))
        }
    }
}

impl TryFrom<&git::Branch<'_>> for Refname {
    type Error = Error;

    fn try_from(value: &git::Branch<'_>) -> std::result::Result<Self, Self::Error> {
        if value.is_remote() {
            Ok(Self::Remote(RemoteRefname::try_from(value)?))
        } else {
            Ok(Self::Local(LocalRefname::try_from(value)?))
        }
    }
}

impl fmt::Display for Refname {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Remote(remote) => remote.fmt(f),
            Self::Local(local) => local.fmt(f),
        }
    }
}

impl Serialize for Refname {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Remote(remote) => remote.serialize(serializer),
            Self::Local(local) => local.serialize(serializer),
        }
    }
}

impl<'d> Deserialize<'d> for Refname {
    fn deserialize<D: serde::Deserializer<'d>>(deserializer: D) -> Result<Self, D::Error> {
        let name = String::deserialize(deserializer)?;
        name.parse().map_err(serde::de::Error::custom)
    }
}
