use serde::{Deserialize, Deserializer};
use std::{ops::Deref, str::FromStr};

/// A type that deserializes a hexadecimal hash into an object id automatically.
#[derive(Debug, Clone, Copy)]
pub struct HexHash(gix::ObjectId);

impl From<HexHash> for gix::ObjectId {
    fn from(value: HexHash) -> Self {
        value.0
    }
}

impl Deref for HexHash {
    type Target = gix::ObjectId;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<'de> Deserialize<'de> for HexHash {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let hex = String::deserialize(deserializer)?;
        gix::ObjectId::from_str(&hex)
            .map(HexHash)
            .map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_hash() {
        let hex_str = "5c69907b1244089142905dba380371728e2e8160";
        let expected = gix::ObjectId::from_str(hex_str).expect("valid SHA1 hex-string");
        let actual =
            serde_json::from_str::<HexHash>(&format!("\"{hex_str}\"")).expect("input is valid");
        assert_eq!(actual.0, expected);
    }
}
