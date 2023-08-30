use std::{fmt, str::FromStr};

#[derive(Debug, PartialEq, Copy, Clone, Hash, Eq)]
pub struct Oid {
    oid: git2::Oid,
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
