use crate::HunkHash;
use gix::bstr::ByteSlice;
use std::hash::Hasher;

pub(crate) trait PaniclessSubtraction<T> {
    /// Subtract on T from another or fail if there is an overflow.
    fn sub_or_err(&self, b: T) -> anyhow::Result<u32>;
}

impl PaniclessSubtraction<u32> for u32 {
    fn sub_or_err(&self, b: u32) -> anyhow::Result<u32> {
        self.checked_sub(b)
            .ok_or_else(|| anyhow::anyhow!("Subtraction overflow: {} - {}.", self, b))
    }
}

/// Calculate as hash for a `universal_diff`.
// TODO: see if this should be avoided entirely here as the current impl would allow for hash collisions.
pub fn hash_lines(universal_diff: impl AsRef<[u8]>) -> HunkHash {
    let diff = universal_diff.as_ref();
    assert!(
        diff.starts_with(b"@@"),
        "BUG: input mut be a universal diff"
    );
    let mut ctx = rustc_hash::FxHasher::default();
    diff.lines_with_terminator()
        .skip(1) // skip the first line which is the diff header.
        .for_each(|line| ctx.write(line));
    ctx.finish()
}
