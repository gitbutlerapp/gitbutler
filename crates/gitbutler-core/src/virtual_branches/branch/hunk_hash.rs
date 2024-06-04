use std::{
    fmt,
    ops::{Deref, DerefMut},
};

#[repr(transparent)]
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct HunkHash(md5::Digest);

impl Default for HunkHash {
    fn default() -> Self {
        HunkHash(md5::Digest([0; 16]))
    }
}

impl From<md5::Digest> for HunkHash {
    fn from(digest: md5::Digest) -> Self {
        HunkHash(digest)
    }
}

impl From<HunkHash> for md5::Digest {
    fn from(hash: HunkHash) -> Self {
        hash.0
    }
}

impl serde::Serialize for HunkHash {
    fn serialize<S: serde::Serializer>(
        &self,
        serializer: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        serializer.serialize_str(hex::encode(self.0 .0).as_str())
    }
}

impl<'de> serde::Deserialize<'de> for HunkHash {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        let mut buf = [0u8; 16];
        hex::decode_to_slice(s, &mut buf).map_err(serde::de::Error::custom)?;
        Ok(md5::Digest(buf).into())
    }
}

impl AsRef<md5::Digest> for HunkHash {
    fn as_ref(&self) -> &md5::Digest {
        &self.0
    }
}

impl Deref for HunkHash {
    type Target = md5::Digest;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for HunkHash {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl fmt::Debug for HunkHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(&self.0, f)
    }
}

impl fmt::LowerHex for HunkHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::LowerHex::fmt(&self.0, f)
    }
}

impl fmt::UpperHex for HunkHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::UpperHex::fmt(&self.0, f)
    }
}
