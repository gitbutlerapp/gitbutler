use std::{fmt, marker::PhantomData, ops::Deref};

use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// Tagged string is designed to clarify the purpose of strings when used as a return type
pub struct TaggedString<T>(String, PhantomData<T>);

impl<T> From<String> for TaggedString<T> {
    fn from(value: String) -> Self {
        TaggedString(value, PhantomData)
    }
}

impl<T> From<&str> for TaggedString<T> {
    fn from(value: &str) -> Self {
        TaggedString(value.to_string(), PhantomData)
    }
}

impl<T> Deref for TaggedString<T> {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<'de, T> Deserialize<'de> for TaggedString<T> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        String::deserialize(deserializer).map(Into::into)
    }
}

impl<T> Serialize for TaggedString<T> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.0.serialize(serializer)
    }
}

impl<T> fmt::Display for TaggedString<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl<T> fmt::Debug for TaggedString<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

pub struct _ReferenceName;
/// The name of a reference ie. `refs/heads/master`
pub type ReferenceName = TaggedString<_ReferenceName>;
