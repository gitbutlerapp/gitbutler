use bstr::{BStr, BString, ByteSlice};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::ops::{Deref, DerefMut};

/// A form of `BString` for use in structures that are going to be serialized for the frontend as string.
///
/// ### Note
///
/// `BString` provides its own serialize implementation, but serializes as list of bytes, something that
/// would break the UI. Thus, whenever `BString` is involved, a custom serialization or this type
/// will be required.
#[derive(Clone, Debug, Ord, PartialOrd, Eq, PartialEq, Hash, Default)]
pub struct BStringForFrontend(BString);

impl Serialize for BStringForFrontend {
    fn serialize<S>(&self, s: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.0.to_str_lossy().serialize(s)
    }
}

impl<'de> Deserialize<'de> for BStringForFrontend {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        String::deserialize(deserializer).map(Into::into)
    }
}

impl Deref for BStringForFrontend {
    type Target = BString;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for BStringForFrontend {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl AsRef<[u8]> for BStringForFrontend {
    fn as_ref(&self) -> &[u8] {
        self.0.as_ref()
    }
}

impl AsRef<BStr> for BStringForFrontend {
    fn as_ref(&self) -> &BStr {
        self.0.as_ref()
    }
}

impl From<String> for BStringForFrontend {
    fn from(value: String) -> Self {
        BStringForFrontend(value.into())
    }
}

impl From<BString> for BStringForFrontend {
    fn from(value: BString) -> Self {
        BStringForFrontend(value)
    }
}

impl From<&BStr> for BStringForFrontend {
    fn from(value: &BStr) -> Self {
        BStringForFrontend(value.into())
    }
}

/// Primarily for tests
impl From<&str> for BStringForFrontend {
    fn from(value: &str) -> Self {
        BStringForFrontend(value.into())
    }
}

impl PartialEq<&str> for BStringForFrontend {
    fn eq(&self, other: &&str) -> bool {
        self.0.eq(other)
    }
}
