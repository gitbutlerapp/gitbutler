use std::{fmt, hash::Hash, str};

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use uuid::Uuid;

/// A generic UUID, to be specialised for each kind of UUID.
///
/// `Default` is implemented to generate a new UUID
/// via [`Uuid::new_v4`].
#[derive(Copy, Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct Id<const KIND: char>(Uuid);

impl<const KIND: char> Id<KIND> {
    /// Generate a new ID.
    #[must_use]
    pub fn generate() -> Self {
        Id(Uuid::new_v4())
    }

    /// Create a new ID that is stable. Only useful for testing.
    pub fn from_number_for_testing(stable_id: u128) -> Self {
        Id(Uuid::from_u128(stable_id))
    }
}

impl<const KIND: char> From<Uuid> for Id<KIND> {
    fn from(value: Uuid) -> Self {
        Self(value)
    }
}

impl<const KIND: char> From<Id<KIND>> for Uuid {
    fn from(val: Id<KIND>) -> Self {
        val.0
    }
}

impl<'de, const KIND: char> Deserialize<'de> for Id<KIND> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Uuid::deserialize(deserializer).map(Into::into)
    }
}

impl<const KIND: char> Serialize for Id<KIND> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.0.serialize(serializer)
    }
}

impl<const KIND: char> fmt::Display for Id<KIND> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl<const KIND: char> fmt::Debug for Id<KIND> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl<const KIND: char> str::FromStr for Id<KIND> {
    type Err = uuid::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Uuid::parse_str(s).map(Into::into)
    }
}
