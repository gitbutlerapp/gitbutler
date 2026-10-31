use std::{fmt, str::FromStr};

use gix::refs::FullNameRef;
use serde::{Deserialize, Serialize};

use super::error::Error;

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct Refname {
    /// The remote name, such as "origin", or `None` for a local branch.
    remote: Option<String>,
    /// contains name of the branch, e.x. "master" or "main"
    // TODO(ST): use `BString` for this, or maybe figure out if there could
    //           be better abstractions for `Refname`, or a better name for the type.
    branch: String,
}

impl Refname {
    pub fn new(remote: &str, branch: &str) -> Refname {
        Refname {
            remote: Some(remote.to_string()),
            branch: branch.to_string(),
        }
    }

    pub fn with_branch(&self, branch: &str) -> Self {
        Self {
            branch: branch.to_string(),
            remote: self.remote.clone(),
        }
    }

    pub fn branch(&self) -> &str {
        &self.branch
    }

    pub fn remote(&self) -> Option<&str> {
        self.remote.as_deref()
    }
}

impl fmt::Display for Refname {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.remote {
            Some(remote) => write!(f, "refs/remotes/{remote}/{}", self.branch),
            None => write!(f, "refs/heads/{}", self.branch),
        }
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

impl FromStr for Refname {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let name: gix::refs::FullName = value
            .try_into()
            .map_err(|_| Error::InvalidName(value.to_string()))?;

        if name.category() == Some(gix::refs::Category::LocalBranch) {
            return Ok(Refname {
                remote: None,
                branch: value
                    .strip_prefix("refs/heads/")
                    .expect("local branch category guarantees the prefix")
                    .to_string(),
            });
        }
        if name.category() != Some(gix::refs::Category::RemoteBranch) {
            return Err(Error::NotRemote(value.to_string()));
        }

        let value = value
            .strip_prefix("refs/remotes/")
            .expect("remote branch category guarantees the prefix");

        // TODO(ST): the remote name cannot be assumed to *not* contain slashes, but the refspec
        //           would be '+refs/heads/*:refs/remotes/multi/slash/remote/*' which allows to extract
        //           the right remote name. However, for that we need the local branch, which
        //           has the remote name configured in plain text. Technically, it doesn't even have
        //           to match the refspec, so this abstraction is very dangerous.
        if let Some((remote, branch)) = value.split_once('/') {
            Ok(Refname {
                remote: Some(remote.to_string()),
                branch: branch.to_string(),
            })
        } else {
            Err(Error::InvalidName(value.to_string()))
        }
    }
}

impl PartialEq<FullNameRef> for Refname {
    fn eq(&self, other: &FullNameRef) -> bool {
        let Some((category, shortname)) = other.category_and_short_name() else {
            return false;
        };
        match &self.remote {
            Some(remote) => {
                category == gix::refs::Category::RemoteBranch
                    && shortname
                        .strip_prefix(remote.as_bytes())
                        .and_then(|rest| rest.strip_suffix(self.branch.as_bytes()))
                        .is_some_and(|rest| rest == b"/")
            }
            None => {
                category == gix::refs::Category::LocalBranch && shortname == self.branch.as_bytes()
            }
        }
    }
}

impl TryFrom<&Refname> for gix::refs::FullName {
    type Error = anyhow::Error;

    fn try_from(value: &Refname) -> std::result::Result<Self, Self::Error> {
        Ok(value.to_string().try_into()?)
    }
}

impl TryFrom<Refname> for gix::refs::FullName {
    type Error = anyhow::Error;

    fn try_from(value: Refname) -> std::result::Result<Self, Self::Error> {
        (&value).try_into()
    }
}
