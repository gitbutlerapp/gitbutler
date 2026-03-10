use std::{fmt, str};

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use uuid::Uuid;

/// A UUID-based legacy project identifier carried for compatibility while project storage
/// still lives in `gitbutler-project`.
#[derive(Copy, Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct LegacyProjectId(Uuid);

impl LegacyProjectId {
    /// Create a stable ID for tests.
    pub fn from_number_for_testing(stable_id: u128) -> Self {
        Self(Uuid::from_u128(stable_id))
    }
}

impl From<Uuid> for LegacyProjectId {
    fn from(value: Uuid) -> Self {
        Self(value)
    }
}

impl From<LegacyProjectId> for Uuid {
    fn from(value: LegacyProjectId) -> Self {
        value.0
    }
}

impl<'de> Deserialize<'de> for LegacyProjectId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Uuid::deserialize(deserializer).map(Into::into)
    }
}

impl Serialize for LegacyProjectId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.0.serialize(serializer)
    }
}

impl fmt::Display for LegacyProjectId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl fmt::Debug for LegacyProjectId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl str::FromStr for LegacyProjectId {
    type Err = uuid::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Uuid::parse_str(s).map(Into::into)
    }
}
