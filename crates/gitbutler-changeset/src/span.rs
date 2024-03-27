/// A line-based span of text.
///
/// All line spans are at least one line long.
/// Line numbers are 0-based.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LineSpan {
    start: usize,
    end: usize, // Exclusive
}

impl LineSpan {
    /// Creates a new line span from the given start and end lines.
    /// Note that line numbers are zero-based, and the ending
    /// line number is exclusive.
    ///
    /// # Panics
    ///
    /// Panics if the start line is greater than or equal to the end line.
    #[must_use]
    pub fn new(start: usize, end: usize) -> Self {
        assert!(
            start <= end,
            "start line must be less than or equal to the end line"
        );
        Self { start, end }
    }

    /// The starting line of the span. Zero-based.
    #[inline]
    #[must_use]
    pub fn start(&self) -> usize {
        self.start
    }

    /// The ending line of the span. Zero-based, exclusive.
    #[inline]
    #[must_use]
    pub fn end(&self) -> usize {
        self.end
    }

    /// Gets the line count from the span
    #[inline]
    #[must_use]
    pub fn line_count(&self) -> usize {
        self.end - self.start
    }

    /// Checks if the span is empty.
    #[inline]
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.start == self.end
    }

    /// Returns true if the given span intersects with this span.
    #[inline]
    #[must_use]
    pub fn intersects(&self, other: &Self) -> bool {
        other.start < self.end && other.end > self.start
    }

    /// Extracts the lines from the span from the given text.
    /// The final line ending (if any) is included.
    ///
    /// Also returns the character offsets (exclusive).
    ///
    /// If the span is empty (i.e. start == end), or if the start
    /// line starts after the last line, this will return `None`.
    ///
    /// If the end line is after the last line, it will be clamped
    /// to the last line of the input text.
    ///
    /// # Panics
    /// Panics if the span's start > end.
    #[must_use]
    pub fn extract<'a>(&self, text: &'a str) -> Option<(&'a str, usize, usize)> {
        debug_assert!(self.end >= self.start);

        if text.is_empty() || self.start == self.end {
            return None;
        }

        let mut start_offset = if self.start == 0 { Some(0) } else { None };
        let mut current_line = 0;
        let mut end_offset = None;

        for (i, _) in text.char_indices().filter(|(_, c)| *c == '\n') {
            current_line += 1;

            if current_line == self.start {
                start_offset = Some(i + 1);
            } else if current_line == self.end {
                debug_assert!(start_offset.is_some());
                end_offset = Some(i + 1);
                break;
            }
        }

        start_offset.map(|start_offset| {
            let end_offset = end_offset
                .map(|i| if i > text.len() { i - 1 } else { i })
                .unwrap_or_else(|| text.len());
            (&text[start_offset..end_offset], start_offset, end_offset)
        })
    }
}
