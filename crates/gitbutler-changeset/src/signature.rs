//! A note about serialization:
//!
//! You may serialize the signature however you'd like; it's just a fixed-length byte array.
//! We _would_ support `serde` but currently fixed-length arrays have terrible, quasi-non-existent
//! support.
//!
//! Related issues:
//!
//! - <https://github.com/serde-rs/serde/issues/2120>
//! - <https://github.com/serde-rs/serde/issues/1937>
//! - <https://github.com/serde-rs/serde/issues/1272>
//!
//! If/when those are fixed, we should be able to (trivially) add `serde` support.
//! Otherwise, neither the length prefix imposed by `(de)serialize_bytes()` nor the
//! terrible compaction and optimization of `(de)serialize_tuple()` are acceptable.

const BITS: usize = 3;
const SHIFT: usize = 8 - BITS;
const FINGERPRINT_ENTRIES: usize = (1 << BITS) * (1 << BITS);
const FINGERPRINT_BYTES: usize = FINGERPRINT_ENTRIES * ::core::mem::size_of::<SigBucket>();
const TOTAL_BYTES: usize = 1 + 4 + FINGERPRINT_BYTES; // we encode a version byte and a 4-byte length at the beginning

// NOTE: This is not efficient if `SigBucket` is 1 byte (u8).
// NOTE: If `SigBucket` is changed to a u8, then the implementation
// NOTE: *should* be updated to eschew the byte conversion and use
// NOTE: slices directly.
type SigBucket = u16;

/// Similarity signatures are fixed-width bigram histograms from the
/// [Sørensen-Dice coefficient algorithm](https://en.wikipedia.org/wiki/S%C3%B8rensen%E2%80%93Dice_coefficient).
/// They act as fixed-length fingerprints for a file's contents, usable
/// to check similarity between two hunks, using the fingerprint of the
/// old hunk and the string contents of a new hunk.
///
/// This implementation is based on the crate [`strsim`](https://crates.io/crates/strsim),
/// but has been modified to be two-step (first, create a signature from
/// a string, then compare the signature to another string), as well as
/// to use bytes instead of `char`s, quantize the input bytes, and then
/// to use saturating arithmetic to avoid overflows and reduce total
/// memory usage.
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct Signature([u8; TOTAL_BYTES]);

impl Signature {
    /// Creates a new signature from a byte array.
    #[inline]
    #[must_use]
    pub fn new(bytes: [u8; TOTAL_BYTES]) -> Self {
        Self(bytes)
    }

    /// Returns the similarity signature as a byte array.
    ///
    /// **NOTE:** Do not inspect the contents of this array,
    /// or assume anything about its contents. It is an
    /// implementation detail and may change at any time.
    #[inline]
    #[must_use]
    pub fn as_bytes(&self) -> &[u8; TOTAL_BYTES] {
        &self.0
    }

    /// Scores this signature against a string.
    /// Values are between 0.0 and 1.0, with 1.0 being
    /// a perfect match.
    ///
    /// Typically, values below 0.95 are pretty indicative
    /// of unrelated hunks.
    ///
    /// # Panics
    ///
    /// Will panic if the signature has an unsupported version
    /// byte.
    ///
    /// # Security
    ///
    /// This function is not fixed-time, and may leak information
    /// about the signature or the original file contents.
    ///
    /// Do not use for any security-related purposes.
    #[must_use]
    pub fn score_str<S: AsRef<str>>(&self, other: S) -> f64 {
        assert!(self.0[0] == 0, "unsupported signature version");

        let original_length = u32::from_le_bytes(self.0[1..5].try_into().expect("invalid length"));
        if original_length < 2 {
            return 0.0;
        }

        let other = other.as_ref();
        let other_string: String = other.chars().filter(|&x| !char::is_whitespace(x)).collect();
        let other = other_string.as_bytes();

        if other.len() < 2 {
            return 0.0;
        }

        let mut matching_bigrams: usize = 0;

        let mut self_buckets = self.bucket_iter().collect::<Vec<_>>();

        for (left, right) in bigrams(other) {
            let left = left >> SHIFT;
            let right = right >> SHIFT;
            let index = ((left as usize) << BITS) | (right as usize);
            if self_buckets[index] > 0 {
                self_buckets[index] -= 1;
                matching_bigrams += 1;
            }
        }

        #[allow(clippy::cast_precision_loss)]
        {
            (2 * matching_bigrams) as f64 / (original_length as usize + other.len() - 2) as f64
        }
    }

    fn bucket_iter(&self) -> impl Iterator<Item = SigBucket> + '_ {
        unsafe {
            self.0[(TOTAL_BYTES - FINGERPRINT_BYTES)..]
                .as_chunks_unchecked::<{ ::core::mem::size_of::<SigBucket>() }>()
                .iter()
                .map(|ch: &[u8; ::core::mem::size_of::<SigBucket>()]| SigBucket::from_le_bytes(*ch))
        }
    }
}

impl<S: AsRef<str>> From<S> for Signature {
    #[inline]
    fn from(source: S) -> Self {
        let source = source.as_ref();
        let source_string: String = source
            .chars()
            .filter(|&x| !char::is_whitespace(x))
            .collect();
        let source = source_string.as_bytes();

        let source_len: u32 = source
            .len()
            .try_into()
            .expect("strings with a byte-length above u32::MAX are not supported");

        let mut bytes = [0; TOTAL_BYTES];
        bytes[0] = 0; // version byte (0)
        bytes[1..5].copy_from_slice(&source_len.to_le_bytes()); // next 4 bytes are the length

        if source_len >= 2 {
            let mut buckets = [0 as SigBucket; FINGERPRINT_ENTRIES];

            for (left, right) in bigrams(source) {
                let left = left >> SHIFT;
                let right = right >> SHIFT;
                let index = ((left as usize) << BITS) | (right as usize);
                buckets[index] = buckets[index].saturating_add(1);
            }

            // NOTE: This is not efficient if `SigBucket` is 1 byte (u8).
            let mut offset = TOTAL_BYTES - FINGERPRINT_BYTES;
            for bucket in buckets {
                let start = offset;
                let end = start + ::core::mem::size_of::<SigBucket>();
                bytes[start..end].copy_from_slice(&bucket.to_le_bytes());
                offset = end;
            }
        }

        Self(bytes)
    }
}

/// Copies the passed bytes twice and zips them together with a one-byte offset.
/// This produces an iterator of the bigrams (pairs of consecutive bytes) in the input.
/// For example, the bigrams of 1, 2, 3, 4, 5 would be (1, 2), (2, 3), (3, 4), (4, 5).
#[inline]
fn bigrams(s: &[u8]) -> impl Iterator<Item = (u8, u8)> + '_ {
    s.iter().copied().zip(s.iter().skip(1).copied())
}
