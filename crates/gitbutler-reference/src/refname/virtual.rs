use std::{fmt, str::FromStr};

use serde::{Deserialize, Serialize};

use super::error::Error;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Refname {
    /// contains slug of the virtual branch name.
    /// Note that `Display` is implemented to put `refs/gitbutler/` in front of `branch`
    /// so it's assume to be a valid branch name otherwise.
    pub branch: String,
}

impl Refname {
    pub fn branch(&self) -> &str {
        &self.branch
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
        write!(f, "refs/gitbutler/{}", self.branch)
    }
}

impl FromStr for Refname {
    type Err = Error;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if !value.starts_with("refs/gitbutler/") {
            return Err(Error::NotLocal(value.to_string()));
        }

        if let Some(branch) = value.strip_prefix("refs/gitbutler/") {
            Ok(Self {
                branch: branch.to_string(),
            })
        } else {
            Err(Error::InvalidName(value.to_string()))
        }
    }
}
