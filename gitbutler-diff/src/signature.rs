//! A note about serialization:
//!
//! You may serialize the signature however you'd like; it's just a fixed-length byte array.
//! We _would_ support `serde` but currently fixed-length arrays have terrible, quasi-non-existent
//! support.
//!
//! Related issues:
//!
//! - https://github.com/serde-rs/serde/issues/2120
//! - https://github.com/serde-rs/serde/issues/1937
//! - https://github.com/serde-rs/serde/issues/1272
//!
//! If/when those are fixed, we should be able to (trivially) add `serde` support.
//! Otherwise, neither the length prefix imposed by `(de)serialize_bytes()` nor the
//! terrible compaction and optimization of `(de)serialize_tuple()` are acceptable.

const BITS: usize = 3;
const SHIFT: usize = 8 - BITS;
const SIG_ENTRIES: usize = (1 << BITS) * (1 << BITS);
const SIG_BYTES: usize = SIG_ENTRIES * ::core::mem::size_of::<SigBucket>();
const TOTAL_BYTES: usize = SIG_BYTES + 4 + 1; // we encode a 4-byte length at the beginning, along with a version byte

// NOTE: This is not efficient if `SigBucket` is 1 byte (u8).
// NOTE: If `SigBucket` is changed to a u8, then the implementation
// NOTE: *should* be updated to eschew the byte conversion and use
// NOTE: slices directly.
type SigBucket = u16;

/// Similarity signatures are fixed-width bigram histograms
/// from the Sorenson-Dice coefficient algorithm. They act
/// as fixed-length fingerprints for a file's contents,
/// usable to check similarity between two hunks, using the
/// fingerprint of the old hunk and the string contents
/// of a new hunk.
#[derive(Clone, Debug, Hash, PartialEq, Eq)]
pub struct Signature([u8; TOTAL_BYTES]);

impl Signature {
    /// Creates a new signature from a byte array.
    #[inline]
    pub fn new(bytes: [u8; TOTAL_BYTES]) -> Self {
        Self(bytes)
    }

    /// Returns the similarity signature as a byte array.
    ///
    /// **NOTE:** Do not inspect the contents of this array,
    /// or assume anything about its contents. It is an
    /// implementation detail and may change at any time.
    #[inline]
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
    pub fn score_str<S: AsRef<str>>(&self, s: S) -> f64 {
        if self.0[0] != 0 {
            panic!("unsupported signature version");
        }

        let original_length = u32::from_le_bytes(self.0[1..5].try_into().unwrap());

        let s = s.as_ref();

        let s_s: String = s.chars().filter(|&x| !char::is_whitespace(x)).collect();
        let s = s_s.as_bytes();

        if original_length < 2 || s.len() < 2 {
            return 0.0;
        }

        let mut intersection_size = 0usize;

        let mut wb = self.bucket_iter().collect::<Vec<_>>();

        for (b1, b2) in bigrams(s) {
            let b1 = b1 >> SHIFT;
            let b2 = b2 >> SHIFT;
            let ix = ((b1 as usize) << BITS) | (b2 as usize);
            if wb[ix] > 0 {
                wb[ix] = wb[ix].saturating_sub(1);
                intersection_size += 1;
            }
        }

        (2 * intersection_size) as f64 / (original_length as usize + s.len() - 2) as f64
    }

    fn bucket_iter(&self) -> impl Iterator<Item = SigBucket> + '_ {
        unsafe {
            self.0[(TOTAL_BYTES - SIG_BYTES)..]
                .as_chunks_unchecked::<{ ::core::mem::size_of::<SigBucket>() }>()
                .iter()
                .map(|ch: &[u8; ::core::mem::size_of::<SigBucket>()]| SigBucket::from_le_bytes(*ch))
        }
    }
}

impl<S: AsRef<str>> From<S> for Signature {
    #[inline]
    fn from(s: S) -> Self {
        let s = s.as_ref();

        let a_s: String = s.chars().filter(|&x| !char::is_whitespace(x)).collect();
        let a = a_s.as_bytes();

        let a_len: u32 = a
            .len()
            .try_into()
            .expect("strings with a byte-length above u32::MAX are not supported");

        let mut a_res = [0; TOTAL_BYTES];
        a_res[0] = 0; // version byte
        a_res[1..5].copy_from_slice(&a_len.to_le_bytes()); // length

        if a_len >= 2 {
            let mut a_bigrams = [0 as SigBucket; SIG_ENTRIES];

            for (b1, b2) in bigrams(a) {
                let b1 = b1 >> SHIFT;
                let b2 = b2 >> SHIFT;
                let encoded_bigram = ((b1 as usize) << BITS) | (b2 as usize);
                a_bigrams[encoded_bigram] = a_bigrams[encoded_bigram].saturating_add(1);
            }

            // NOTE: This is not efficient if `SigBucket` is 1 byte (u8).
            let mut offset = TOTAL_BYTES - SIG_BYTES;
            for bucket in a_bigrams {
                let start = offset;
                let end = start + ::core::mem::size_of::<SigBucket>();
                a_res[start..end].copy_from_slice(&bucket.to_le_bytes());
                offset = end;
            }
        }

        Self(a_res)
    }
}

#[inline]
fn bigrams(s: &[u8]) -> impl Iterator<Item = (u8, u8)> + '_ {
    s.iter().copied().zip(s.iter().skip(1).copied())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn score_signature() {
        let sig = Signature::from("hello world");

        macro_rules! assert_score {
            ($s:expr, $e:expr) => {
                if (sig.score_str($s) - $e).abs() >= 0.1 {
                    panic!(
                        "expected score of {} for string {:?}, got {}",
                        $e,
                        $s,
                        sig.score_str($s)
                    );
                }
            };
        }

        // NOTE: The scores here are not exact, but are close enough
        // to be useful for testing purposes, hence why some have the same
        // "score" but different strings.
        assert_score!("hello world", 1.0);
        assert_score!("hello world!", 0.95);
        assert_score!("hello world!!", 0.9);
        assert_score!("hello world!!!", 0.85);
        assert_score!("hello world!!!!", 0.8);
        assert_score!("hello world!!!!!", 0.75);
        assert_score!("hello world!!!!!!", 0.7);
        assert_score!("hello world!!!!!!!", 0.65);
        assert_score!("hello world!!!!!!!!", 0.62);
        assert_score!("hello world!!!!!!!!!", 0.6);
        assert_score!("hello world!!!!!!!!!!", 0.55);
    }

    #[test]
    fn score_ignores_whitespace() {
        let sig = Signature::from("hello world");

        macro_rules! assert_score {
            ($s:expr, $e:expr) => {
                if (sig.score_str($s) - $e).abs() >= 0.1 {
                    panic!(
                        "expected score of {} for string {:?}, got {}",
                        $e,
                        $s,
                        sig.score_str($s)
                    );
                }
            };
        }

        assert_score!("hello world", 1.0);
        assert_score!("hello world ", 1.0);
        assert_score!("hello\nworld ", 1.0);
        assert_score!("hello\n\tworld ", 1.0);
        assert_score!("\t\t  hel lo\n\two rld \t\t", 1.0);
    }

    const TEXT1: &str = include_str!("../fixture/text1.txt");
    const TEXT2: &str = include_str!("../fixture/text2.txt");
    const TEXT3: &str = include_str!("../fixture/text3.txt");
    const CODE1: &str = include_str!("../fixture/code1.txt");
    const CODE2: &str = include_str!("../fixture/code2.txt");
    const CODE3: &str = include_str!("../fixture/code3.txt");
    const CODE4: &str = include_str!("../fixture/code4.txt");
    const LARGE1: &str = include_str!("../fixture/large1.txt");
    const LARGE2: &str = include_str!("../fixture/large2.txt");

    macro_rules! real_test {
        ($a: ident, $b: ident, are_similar) => {
            paste::paste! {
                #[test]
                #[allow(non_snake_case)]
                fn [<test_ $a _ $b _are_similar>]() {
                    let a = Signature::from($a);
                    let b = Signature::from($b);
                    assert!(a.score_str($b) >= 0.95);
                    assert!(b.score_str($a) >= 0.95);
                }
            }
        };
        ($a: ident, $b: ident, are_not_similar) => {
            paste::paste! {
                #[test]
                #[allow(non_snake_case)]
                fn [<test_ $a _ $b _are_not_similar>]() {
                    let a = Signature::from($a);
                    let b = Signature::from($b);
                    assert!(a.score_str($b) < 0.95);
                    assert!(b.score_str($a) < 0.95);
                }
            }
        };
    }

    // Only similar pairs:
    // - TEXT1, TEXT2
    // - CODE1, CODE2
    // - LARGE1, LARGE2
    real_test!(TEXT1, TEXT2, are_similar);
    real_test!(CODE1, CODE2, are_similar);
    real_test!(LARGE1, LARGE2, are_similar);

    // Check all other combos
    real_test!(TEXT1, TEXT3, are_not_similar);
    real_test!(TEXT1, CODE1, are_not_similar);
    real_test!(TEXT1, CODE2, are_not_similar);
    real_test!(TEXT1, CODE3, are_not_similar);
    real_test!(TEXT1, CODE4, are_not_similar);
    real_test!(TEXT1, LARGE1, are_not_similar);
    real_test!(TEXT1, LARGE2, are_not_similar);
    real_test!(TEXT2, TEXT3, are_not_similar);
    real_test!(TEXT2, CODE1, are_not_similar);
    real_test!(TEXT2, CODE2, are_not_similar);
    real_test!(TEXT2, CODE3, are_not_similar);
    real_test!(TEXT2, CODE4, are_not_similar);
    real_test!(TEXT2, LARGE1, are_not_similar);
    real_test!(TEXT2, LARGE2, are_not_similar);
    real_test!(TEXT3, CODE1, are_not_similar);
    real_test!(TEXT3, CODE2, are_not_similar);
    real_test!(TEXT3, CODE3, are_not_similar);
    real_test!(TEXT3, CODE4, are_not_similar);
    real_test!(TEXT3, LARGE1, are_not_similar);
    real_test!(TEXT3, LARGE2, are_not_similar);
    real_test!(CODE1, CODE3, are_not_similar);
    real_test!(CODE1, CODE4, are_not_similar);
    real_test!(CODE1, LARGE1, are_not_similar);
    real_test!(CODE1, LARGE2, are_not_similar);
    real_test!(CODE2, CODE3, are_not_similar);
    real_test!(CODE2, CODE4, are_not_similar);
    real_test!(CODE2, LARGE1, are_not_similar);
    real_test!(CODE2, LARGE2, are_not_similar);
    real_test!(CODE3, CODE4, are_not_similar);
    real_test!(CODE3, LARGE1, are_not_similar);
    real_test!(CODE3, LARGE2, are_not_similar);
    real_test!(CODE4, LARGE1, are_not_similar);
    real_test!(CODE4, LARGE2, are_not_similar);
}
