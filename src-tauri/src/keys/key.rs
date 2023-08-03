use std::{fmt, str::FromStr};

use ed25519_dalek::{
    pkcs8::{
        spki::der::pem::LineEnding, DecodePrivateKey, DecodePublicKey, EncodePrivateKey,
        EncodePublicKey, Error as PKSC8Error,
    },
    SigningKey, VerifyingKey,
};
use rand::rngs::OsRng;
use serde::{Deserialize, Serialize};

#[derive(Debug)]
pub struct PrivateKey(SigningKey);

impl PrivateKey {
    pub fn generate() -> Self {
        Self::default()
    }

    pub fn public_key(&self) -> PublicKey {
        PublicKey::from(self)
    }
}

impl Default for PrivateKey {
    fn default() -> Self {
        let mut csprng = OsRng;
        let signing_key: SigningKey = SigningKey::generate(&mut csprng);
        Self(signing_key)
    }
}

impl PartialEq for PrivateKey {
    fn eq(&self, other: &Self) -> bool {
        self.0.to_bytes().eq(&other.0.to_bytes())
    }
}

impl Serialize for PrivateKey {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.to_string().serialize(serializer)
    }
}

impl FromStr for PrivateKey {
    type Err = PKSC8Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let key = SigningKey::from_pkcs8_pem(s)?;
        Ok(Self(key))
    }
}

impl fmt::Display for PrivateKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0
            .to_pkcs8_pem(LineEnding::default())
            .map_err(|_| fmt::Error)?
            .fmt(f)
    }
}

impl<'de> Deserialize<'de> for PrivateKey {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Self::from_str(&s).map_err(serde::de::Error::custom)
    }
}

#[derive(Debug)]
pub struct PublicKey(VerifyingKey);

impl From<&PrivateKey> for PublicKey {
    fn from(value: &PrivateKey) -> Self {
        Self(value.0.verifying_key())
    }
}

impl PartialEq for PublicKey {
    fn eq(&self, other: &Self) -> bool {
        self.0.to_bytes().eq(&other.0.to_bytes())
    }
}

impl fmt::Display for PublicKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0
            .to_public_key_pem(LineEnding::default())
            .map_err(|_| fmt::Error)?
            .fmt(f)
    }
}

impl FromStr for PublicKey {
    type Err = PKSC8Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let key = VerifyingKey::from_public_key_pem(s)?;
        Ok(Self(key))
    }
}

impl Serialize for PublicKey {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.to_string().serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for PublicKey {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Self::from_str(s.as_str()).map_err(serde::de::Error::custom)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_from_string_private() {
        let private_key = PrivateKey::generate();
        let serialized = private_key.to_string();
        let deserialized: PrivateKey = serialized.parse().unwrap();
        assert_eq!(private_key, deserialized);
    }

    #[test]
    fn test_to_from_string_public() {
        let private_key = PrivateKey::generate();
        let public_key = private_key.public_key();
        let serialized = public_key.to_string();
        let deserialized: PublicKey = serialized.parse().unwrap();
        assert_eq!(public_key, deserialized);
    }

    #[test]
    fn test_serde_private() {
        let private_key = PrivateKey::generate();
        let serialized = serde_json::to_string(&private_key).unwrap();
        let deserialized: PrivateKey = serde_json::from_str(&serialized).unwrap();
        assert_eq!(private_key, deserialized);
    }

    #[test]
    fn test_serde_public() {
        let private_key = PrivateKey::generate();
        let public_key = private_key.public_key();
        let serialized = serde_json::to_string(&public_key).unwrap();
        let deserialized: PublicKey = serde_json::from_str(&serialized).unwrap();
        assert_eq!(public_key, deserialized);
    }
}
