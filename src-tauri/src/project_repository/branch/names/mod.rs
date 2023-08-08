mod error;
mod local;
mod remote;

use std::{fmt, str::FromStr};

use serde::{Deserialize, Serialize};

pub use error::Error;
pub use local::Name as LocalName;
pub use remote::Name as RemoteName;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Name {
    Remote(RemoteName),
    Local(LocalName),
}

impl Name {
    pub fn branch(&self) -> &str {
        match self {
            Self::Remote(remote) => remote.branch(),
            Self::Local(local) => local.branch(),
        }
    }
}

impl FromStr for Name {
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

impl TryFrom<&git2::Branch<'_>> for Name {
    type Error = Error;

    fn try_from(value: &git2::Branch<'_>) -> std::result::Result<Self, Self::Error> {
        if value.get().is_remote() {
            Ok(Self::Remote(RemoteName::try_from(value)?))
        } else {
            Ok(Self::Local(LocalName::try_from(value)?))
        }
    }
}

impl fmt::Display for Name {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Remote(remote) => remote.fmt(f),
            Self::Local(local) => local.fmt(f),
        }
    }
}

impl Serialize for Name {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Remote(remote) => remote.serialize(serializer),
            Self::Local(local) => local.serialize(serializer),
        }
    }
}

impl<'d> Deserialize<'d> for Name {
    fn deserialize<D: serde::Deserializer<'d>>(deserializer: D) -> Result<Self, D::Error> {
        let name = String::deserialize(deserializer)?;
        name.parse().map_err(serde::de::Error::custom)
    }
}
