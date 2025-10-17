mod error;
mod local;
mod remote;
mod r#virtual;

use std::{fmt, str::FromStr};

pub use error::Error;
pub use local::Refname as LocalRefname;
pub use remote::Refname as RemoteRefname;
use serde::{Deserialize, Serialize};
pub use r#virtual::Refname as VirtualRefname;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Refname {
    Other(String),
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
            Self::Other(_) => None,
            Self::Remote(remote) => Some(remote.branch()),
            Self::Local(local) => Some(local.branch()),
            Self::Virtual(r#virtual) => Some(r#virtual.branch()),
        }
    }

    pub fn simple_name(&self) -> String {
        match self {
            Refname::Virtual(virtual_refname) => virtual_refname.branch().to_string(),
            Refname::Local(local) => local.branch().to_string(),
            Refname::Remote(remote) => remote.fullname(),
            Refname::Other(raw) => raw.to_string(),
        }
    }

    pub fn remote(&self) -> Option<&str> {
        match self {
            Self::Remote(remote) => Some(remote.remote()),
            Self::Local(remote) => remote.remote().map(|remote| remote.remote()),
            _ => None,
        }
    }
}

impl FromStr for Refname {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            value if value.starts_with("refs/remotes/") => Ok(Self::Remote(value.parse()?)),
            value if value.starts_with("refs/heads/") => Ok(Self::Local(value.parse()?)),
            value if value.starts_with("refs/gitbutler/") => Ok(Self::Virtual(value.parse()?)),
            "HEAD" => Ok(Self::Other(value.to_string())),
            value if value.starts_with("refs/") => Ok(Self::Other(value.to_string())),
            _ => Err(Error::InvalidName(value.to_string())),
        }
    }
}

impl TryFrom<&git2::Branch<'_>> for Refname {
    type Error = Error;

    fn try_from(value: &git2::Branch<'_>) -> std::result::Result<Self, Self::Error> {
        if value.get().is_remote() {
            Ok(Self::Remote(RemoteRefname::try_from(value)?))
        } else {
            Ok(Self::Local(LocalRefname::try_from(value)?))
        }
    }
}

impl fmt::Display for Refname {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Other(raw) => raw.fmt(f),
            Self::Remote(remote) => remote.fmt(f),
            Self::Local(local) => local.fmt(f),
            Self::Virtual(r#virtual) => r#virtual.fmt(f),
        }
    }
}

impl Serialize for Refname {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Other(raw) => raw.serialize(serializer),
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
