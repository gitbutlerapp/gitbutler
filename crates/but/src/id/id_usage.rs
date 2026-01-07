use std::collections::HashSet;

use anyhow::bail;

use crate::id::ShortId;

fn divmod(a: usize, b: usize) -> (usize, usize) {
    (a / b, a % b)
}

/// An integer representation of a [ShortId] that starts with g-z).
#[derive(Debug, PartialEq, Eq, Clone, Copy, Default, Hash)]
pub(crate) struct UintId(pub(crate) u16);
impl UintId {
    /// First character: g-z (20 options)
    const FIRST_CHARS: &'static [u8] = b"ghijklmnopqrstuvwxyz";
    /// Subsequent characters: 0-9,a-z (36 options)
    const SUBSEQUENT_CHARS: &'static [u8] = b"0123456789abcdefghijklmnopqrstuvwxyz";
    /// Must be less than this.
    const LIMIT: u16 = 20 * 36 * 37;
    /// String representation must be at most this long.
    pub(crate) const LENGTH_LIMIT: usize = 3;

    /// If self cannot be represented in 3 characters, `00` is returned.
    pub(crate) fn to_short_id(self) -> ShortId {
        let mut result = String::new();

        let (quo, rem) = divmod(self.0 as usize, 20);
        result.push(Self::FIRST_CHARS[rem] as char);
        let (quo, rem) = divmod(quo, 36);
        result.push(Self::SUBSEQUENT_CHARS[rem] as char);
        let (quo, rem) = divmod(quo, 37);
        if quo > 0 {
            // self is too big even for 3 characters.
            return "00".to_string();
        }
        if rem > 0 {
            result.push(Self::SUBSEQUENT_CHARS[rem - 1] as char);
        }

        result
    }
}

/// Lifecycle
impl UintId {
    /// Pick the first 2 to three characters and see if they are a valid `UintId`.
    /// Return `None` for `value` has more than three characters or less than two.
    pub(crate) fn from_name(value: &[u8]) -> Option<Self> {
        let (first_char, second_char, third_char) = match value {
            [a, b] => (a, b, None),
            [a, b, c] => (a, b, Some(c)),
            _ => {
                return None;
            }
        };

        let mut result: usize = 0;

        let index = Self::FIRST_CHARS.iter().position(|e| e == first_char)?;
        result += index;

        let index = Self::SUBSEQUENT_CHARS
            .iter()
            .position(|e| e == second_char)?;
        result += index * 20;

        if let Some(third_char) = third_char {
            let index = Self::SUBSEQUENT_CHARS
                .iter()
                .position(|e| e == third_char)?;
            result += (index + 1) * 20 * 36;
        }

        let result: u16 = result.try_into().expect("below u16::MAX");
        debug_assert!(
            result < Self::LIMIT,
            "BUG: {result} is beyond limit of {}",
            Self::LIMIT
        );
        Some(Self(result))
    }
}

/// A tracker of which [UintId]s have been used.
#[derive(Default, Debug)]
pub(crate) struct IdUsage {
    /// A [UintId] is used if it's in this set.
    uint_ids_used: HashSet<UintId>,
    /// A [UintId] is used if it's less than this number.
    next_uint_id: UintId,
}

impl IdUsage {
    pub(crate) fn mark_used(&mut self, uint_id: UintId) {
        if self.next_uint_id.0 <= uint_id.0 {
            self.uint_ids_used.insert(uint_id);
        }
    }

    pub(crate) fn next_available(&mut self) -> anyhow::Result<UintId> {
        self.forward_next_uint_id_to_not_conflict_with_marked();
        if self.next_uint_id.0 >= UintId::LIMIT {
            bail!("too many IDs");
        }
        let result = self.next_uint_id;
        self.next_uint_id = UintId(self.next_uint_id.0 + 1);
        Ok(result)
    }

    pub(crate) fn forward_next_uint_id_to_not_conflict_with_marked(&mut self) {
        while self.uint_ids_used.remove(&self.next_uint_id) {
            self.next_uint_id = UintId(self.next_uint_id.0 + 1);
        }
    }
}
