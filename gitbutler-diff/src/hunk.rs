use crate::LineSpan;

/// Represents a single change within a file; a hunk is a Git concept
/// for a single block of changes in a file. A hunk is represented
/// by a span of lines in the file, and a replacement string.
#[derive(Debug, Clone, Hash)]
pub struct Hunk {
    source_span: LineSpan,
    replacement: String,
}

impl Hunk {
    /// Creates a new hunk from the given source span and replacement text.
    /// The replacement text should not include the final line ending.
    ///
    /// # Panics
    ///
    /// Panics if the replacement text includes a leading or trailing
    /// newline.
    pub fn new(source_span: LineSpan, replacement: String) -> Self {
        if replacement.starts_with('\n') || replacement.ends_with('\n') {
            panic!("replacement text cannot include leading or trailing newline");
        }

        Self {
            source_span,
            replacement,
        }
    }

    /// The source line span of the hunk; the lines
    /// that are being replaced.
    #[inline]
    pub fn source_span(&self) -> LineSpan {
        self.source_span
    }

    /// The replacement text, in full.
    /// Does not include the final line ending.
    #[inline]
    pub fn replacement(&self) -> &str {
        &self.replacement
    }

    /// The number of lines in the replacement text.
    #[inline]
    pub fn line_count(&self) -> usize {
        self.replacement.lines().count()
    }

    /// Returns a new [`LineSpan`] that represents the
    /// replacement hunks after replacement.
    #[inline]
    pub fn replacement_span(&self) -> LineSpan {
        LineSpan::new(
            self.source_span.start(),
            self.source_span.start() + self.line_count() - 1,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_hunk() {
        let hunk = Hunk::new(LineSpan::new(4, 10), "hello\nworld".to_string());
        assert_eq!(hunk.source_span(), LineSpan::new(4, 10));
        assert_eq!(hunk.replacement(), "hello\nworld");
        assert_eq!(hunk.line_count(), 2);
        assert_eq!(hunk.replacement_span(), LineSpan::new(4, 5));
    }

    #[test]
    #[should_panic]
    fn create_hunk_with_leading_newline() {
        Hunk::new(LineSpan::new(0, 1), "\nhello\nworld".to_string());
    }

    #[test]
    #[should_panic]
    fn create_hunk_with_trailing_newline() {
        Hunk::new(LineSpan::new(0, 1), "hello\nworld\n".to_string());
    }
}
