mod error;
mod local;
mod remote;
mod tag;
mod r#virtual;

use std::{fmt, str::FromStr};

use serde::{Deserialize, Serialize};

pub use error::Error;
pub use local::Refname as LocalRefname;
pub use r#virtual::Refname as VirtualRefname;
pub use remote::Refname as RemoteRefname;
pub use tag::Refname as TagRefname;

use crate::git;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Refname {
    HEAD,
    Tag(TagRefname),
    Remote(RemoteRefname),
    Local(LocalRefname),
    Virtual(VirtualRefname),
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

impl From<VirtualRefname> for Refname {
    fn from(value: VirtualRefname) -> Self {
        Self::Virtual(value)
    }
}

impl From<&VirtualRefname> for Refname {
    fn from(value: &VirtualRefname) -> Self {
        Self::Virtual(value.clone())
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
    pub fn branch(&self) -> Option<&str> {
        match self {
            Self::HEAD | Self::Tag(_) => None,
            Self::Remote(remote) => Some(remote.branch()),
            Self::Local(local) => Some(local.branch()),
            Self::Virtual(r#virtual) => Some(r#virtual.branch()),
        }
    }
}

impl FromStr for Refname {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if value == "HEAD" {
            Ok(Self::HEAD)
        } else if value.starts_with("refs") {
            if value.starts_with("refs/remotes/") {
                Ok(Self::Remote(value.parse()?))
            } else if value.starts_with("refs/heads/") {
                Ok(Self::Local(value.parse()?))
            } else if value.starts_with("refs/gitbutler/") {
                Ok(Self::Virtual(value.parse()?))
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
            Self::HEAD => write!(f, "HEAD"),
            Self::Tag(tag) => tag.fmt(f),
            Self::Remote(remote) => remote.fmt(f),
            Self::Local(local) => local.fmt(f),
            Self::Virtual(r#virtual) => r#virtual.fmt(f),
        }
    }
}

impl Serialize for Refname {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::HEAD => serializer.serialize_str("HEAD"),
            Self::Tag(tag) => tag.serialize(serializer),
            Self::Remote(remote) => remote.serialize(serializer),
            Self::Local(local) => local.serialize(serializer),
            Self::Virtual(r#virtual) => r#virtual.serialize(serializer),
        }
    }
}

impl<'d> Deserialize<'d> for Refname {
    fn deserialize<D: serde::Deserializer<'d>>(deserializer: D) -> Result<Self, D::Error> {
        let name = String::deserialize(deserializer)?;
        name.parse().map_err(serde::de::Error::custom)
    }
}
