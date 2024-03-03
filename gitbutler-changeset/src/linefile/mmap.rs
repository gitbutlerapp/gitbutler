use crate::{CrlfBehavior, LineFile};
use mmap_rs::{Error, Mmap};

/// A [`LineFile`] created from a read-only memory-mapped file.
pub struct MmapLineFile {
    mmap: Mmap,
    line_slices: Vec<(usize, usize)>,
}

impl MmapLineFile {
    /// Creates a new [`MmapLineFile`] from the given memory-mapped file.
    ///
    /// Will attempt to make the Mmap read-only. Upon failure to do so,
    /// returns both the original [`Mmap`] and the error.
    ///
    /// # Panics
    ///
    /// Panics if the document's contents are not valid UTF-8.
    pub fn from_mmap(mmap: Mmap, crlf_behavior: CrlfBehavior) -> Result<Self, (Mmap, Error)> {
        let mmap = mmap.make_read_only()?;
        let mut line_slices = Vec::new();
        // We check it here to avoid having to check it in the loop.
        std::str::from_utf8(mmap.as_ref()).expect("mmap contents are not valid UTF-8");
        MmapLineFile::init_lines(mmap.as_slice(), &mut line_slices, crlf_behavior);
        Ok(Self { mmap, line_slices })
    }

    /// Creates a new [`MmapLineFile`] from the given file path.
    /// **Unsafely** assumes the file is UTF-8 and will not attempt to convert it.
    ///
    /// Will attempt to make the Mmap read-only. Upon failure to do so,
    /// returns both the original [`Mmap`] and the error.
    ///
    /// # Safety
    ///
    /// This function is unsafe because it assumes the file is UTF-8. If it is not,
    /// the behavior is undefined.
    pub unsafe fn from_mmap_unsafe(
        mmap: Mmap,
        crlf_behavior: CrlfBehavior,
    ) -> Result<Self, (Mmap, Error)> {
        let mmap = mmap.make_read_only()?;
        let mut line_slices = Vec::new();
        MmapLineFile::init_lines(mmap.as_slice(), &mut line_slices, crlf_behavior);
        Ok(Self { mmap, line_slices })
    }

    fn init_lines(text: &[u8], lines: &mut Vec<(usize, usize)>, crlf_behavior: CrlfBehavior) {
        text.iter()
            .enumerate()
            .fold((0, false), |(start, cr), (i, c)| {
                if *c == b'\n' {
                    lines.push((
                        start,
                        i - if cr && crlf_behavior == CrlfBehavior::Trim {
                            1
                        } else {
                            0
                        },
                    ));
                    (i + 1, false)
                } else {
                    (start, *c == b'\r')
                }
            });
    }
}

impl<'a> LineFile<'a> for MmapLineFile {
    type LineIterator = impl Iterator<Item = &'a str>;

    #[inline]
    fn line_count(&self) -> usize {
        self.line_slices.len()
    }

    fn extract(&'a self, span: crate::LineSpan) -> Self::LineIterator {
        let mmap_ref = self.mmap.as_ref();

        self.line_slices[span.start()..=span.end()]
            .iter()
            .map(|(start, end)| &mmap_ref[*start..=*end])
            // The unsafe variant has been checked in the constructors.
            // Either they called `from_mmap` and it was checked there,
            // or they called `from_mmap_unsafe` and the caller assumes
            // responsibility for the UTF-8 invariant.
            .map(|bytes| unsafe { std::str::from_utf8_unchecked(bytes) })
    }
}
