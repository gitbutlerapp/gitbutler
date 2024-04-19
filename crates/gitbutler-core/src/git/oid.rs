use std::{fmt, str::FromStr};

use serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq, Copy, Clone, Hash, Eq)]
pub struct Oid {
    oid: git2::Oid,
}

impl Oid {
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, git2::Error> {
        Ok(Self {
            oid: git2::Oid::from_bytes(bytes)?,
        })
    }
}

impl Default for Oid {
    fn default() -> Self {
        git2::Oid::zero().into()
    }
}

impl Serialize for Oid {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.oid.to_string().serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Oid {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        git2::Oid::from_str(&s)
            .map_err(|e| serde::de::Error::custom(format!("invalid oid: {}", e)))
            .map(Into::into)
    }
}

impl fmt::Display for Oid {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.oid.fmt(f)
    }
}

impl FromStr for Oid {
    type Err = git2::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        git2::Oid::from_str(s).map(Into::into)
    }
}

impl From<git2::Oid> for Oid {
    fn from(oid: git2::Oid) -> Self {
        Self { oid }
    }
}

impl From<Oid> for git2::Oid {
    fn from(oid: Oid) -> Self {
        oid.oid
    }
}
