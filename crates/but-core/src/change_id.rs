//! A type representing ChangeIDs in commit headers

use std::{fmt, ops::Deref};

use bstr::{BStr, BString};
use rand::Rng;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// How long a reverse hex change id should be if stored in actual reverse hex
/// utf-8 characters.
const CHANGE_ID_REVERSE_HEX_LEN: usize = 40;
/// How long a reverse hex change id should be if stored compactly in bytes.
///
/// Should be CHANGE_ID_REVERSE_HEX_LEN * 4 (bits a hex char represents) / 8
/// (byte length)
const CHANGE_ID_REVERSE_HEX_BYTE_LEN: usize = CHANGE_ID_REVERSE_HEX_LEN * 4 / 8;

/// Represents a ChangeID. This can be any arbitrary data read from a git
/// header, but is usually a reverse hex string or a uuid.
#[derive(Clone, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct ChangeId(BString);

impl ChangeId {
    /// Creates a ChangeId from a number for testing purposes
    pub fn from_number_for_testing(value: u128) -> Self {
        ChangeId(value.to_string().into())
    }

    /// Creates a random length 40 reverse hex ChangeId
    pub fn generate() -> Self {
        let mut rng = rand::rng();
        let mut bytes = [0_u8; CHANGE_ID_REVERSE_HEX_BYTE_LEN];
        for byte in &mut bytes {
            *byte = rng.random::<u8>();
        }
        let mut out = BString::from(vec![b'z'; CHANGE_ID_REVERSE_HEX_LEN]);
        for (i, byte) in bytes.iter().enumerate() {
            let [a, b] = byte_to_reverse_hex(*byte);
            let i = i * 2;
            out[i] = a as u8;
            out[i + 1] = b as u8;
        }

        Self(out)
    }
}

/// Converts a byte to reverse hex.
///
/// In reverse hex:
/// 0 == z
/// 1 == x
/// ...
fn byte_to_reverse_hex(byte: u8) -> [char; 2] {
    [
        nibble_to_reverse_hex(byte >> 4),
        nibble_to_reverse_hex(byte),
    ]
}

/// Takes the first four characters of a byte and turns them into a single
/// reverse hex char.
fn nibble_to_reverse_hex(nibble: u8) -> char {
    let nibble = nibble & 0x0F;
    (b'z' - nibble) as char
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
    use crate::change_id::{ChangeId, byte_to_reverse_hex, nibble_to_reverse_hex};

    #[test]
    fn nibble_conversion() {
        assert_eq!(nibble_to_reverse_hex(0x0), 'z');
        assert_eq!(nibble_to_reverse_hex(0x1), 'y');
        assert_eq!(nibble_to_reverse_hex(0x2), 'x');
        assert_eq!(nibble_to_reverse_hex(0x3), 'w');
        assert_eq!(nibble_to_reverse_hex(0xc), 'n');
    }

    #[test]
    fn last_four_bytes_are_ignored_in_nibble_conversion() {
        assert_eq!(nibble_to_reverse_hex(0xa0), 'z');
        assert_eq!(nibble_to_reverse_hex(0x2c), 'n');
    }

    #[test]
    fn byte_conversion() {
        assert_eq!(byte_to_reverse_hex(0x00), ['z', 'z']);
        assert_eq!(byte_to_reverse_hex(0xc0), ['n', 'z']);
        assert_eq!(byte_to_reverse_hex(0x0c), ['z', 'n']);
        assert_eq!(byte_to_reverse_hex(0xcc), ['n', 'n']);
    }

    #[test]
    fn generate_returns_a_40_character_string() {
        assert_eq!(ChangeId::generate().to_string().len(), 40);
    }
}
