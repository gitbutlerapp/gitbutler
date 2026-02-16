//! A type representing ChangeIDs in commit headers.

use std::{fmt, ops::Deref};

use bstr::{BStr, BString};
use rand::Rng;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// How long a reverse hex change id should be if stored in actual reverse hex
/// ASCII characters, ranging from `z` to `k`. In accordance with the standard
/// discussed together with the Jujutsu and Gerrit projects, it is 32.
/// <https://lore.kernel.org/git/CAESOdVAspxUJKGAA58i0tvks4ZOfoGf1Aa5gPr0FXzdcywqUUw@mail.gmail.com>
const CHANGE_ID_REVERSE_HEX_LEN: usize = 32;
/// How long a reverse hex change id should be if stored compactly in bytes.
const CHANGE_ID_REVERSE_BYTE_LEN: usize = CHANGE_ID_REVERSE_HEX_LEN / 2;

/// Represents a ChangeID. This can be any arbitrary data read from a git
/// header, but is usually a reverse hex string or a uuid.
///
/// For all intents and purposes, this type acts like a [`BString`].
#[derive(Clone, Default, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct ChangeId(BString);

impl ChangeId {
    /// Creates a ChangeId from a number for testing purposes.
    pub fn from_number_for_testing(value: u128) -> Self {
        ChangeId(value.to_string().into())
    }

    /// Creates a random length 32 reverse hex ChangeId.
    pub fn generate() -> Self {
        let mut rng = rand::rng();
        let bytes: [u8; CHANGE_ID_REVERSE_BYTE_LEN] = rng.random();
        ChangeId::from_bytes(&bytes)
    }

    /// Creates a reverse hex ChangeId from the first 16 elements of `bytes`. If
    /// there are fewer, the created ChangeId is right-padded with 'z'.
    pub fn from_bytes(bytes: &[u8]) -> Self {
        let mut out = vec![b'z'; CHANGE_ID_REVERSE_HEX_LEN];
        for (i, byte) in bytes.iter().take(CHANGE_ID_REVERSE_BYTE_LEN).enumerate() {
            let [a, b] = byte_to_reverse_hex(*byte);
            let i = i * 2;
            out[i] = a;
            out[i + 1] = b;
        }

        Self(out.into())
    }
}

/// Converts a byte to reverse hex bytes.
///
/// In reverse hex:
/// 0 == z
/// 1 == x
/// …
/// 16 = k
/// ...
fn byte_to_reverse_hex(byte: u8) -> [u8; 2] {
    /// Takes the first four bits of a byte and turns them into a single
    /// reverse hex char.
    fn nibble_to_reverse(nibble: u8) -> u8 {
        let nibble = nibble & 0b0000_1111;
        b'z' - nibble
    }
    [nibble_to_reverse(byte >> 4), nibble_to_reverse(byte)]
}

impl Deref for ChangeId {
    type Target = BString;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl fmt::Debug for ChangeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl fmt::Display for ChangeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl<'de> Deserialize<'de> for ChangeId {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        BString::deserialize(deserializer).map(ChangeId)
    }
}

impl Serialize for ChangeId {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.0.serialize(serializer)
    }
}

impl From<&BStr> for ChangeId {
    fn from(value: &BStr) -> Self {
        Self(value.to_owned())
    }
}

impl From<BString> for ChangeId {
    fn from(value: BString) -> Self {
        Self(value)
    }
}

#[cfg(test)]
mod test {
    use crate::change_id::byte_to_reverse_hex;

    #[test]
    fn byte_to_reverse_hex_conversion() {
        assert_eq!(byte_to_reverse_hex_char(0x00), ['z', 'z']);
        assert_eq!(byte_to_reverse_hex_char(0xc0), ['n', 'z']);
        assert_eq!(byte_to_reverse_hex_char(0x0c), ['z', 'n']);
        assert_eq!(byte_to_reverse_hex_char(0xc0), ['n', 'z']);
        assert_eq!(byte_to_reverse_hex_char(0xcc), ['n', 'n']);
        assert_eq!(byte_to_reverse_hex_char(0xff), ['k', 'k']);
    }

    fn byte_to_reverse_hex_char(byte: u8) -> [char; 2] {
        let [a, b] = byte_to_reverse_hex(byte);
        [a as char, b as char]
    }
}
