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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn span_new() {
        for s in 0..20 {
            for e in s + 1..=20 {
                let span = LineSpan::new(s, e);
                assert_eq!(span.start(), s);
                assert_eq!(span.end(), e);
            }
        }
    }

    #[test]
    fn span_extract() {
        let lines = [
            "Hello, world!",
            "This is a test.",
            "This is another test.\r",
            "This is a third test.\r",
            "This is a fourth test.",
            "This is a fifth test.\r",
            "This is a sixth test.",
            "This is a seventh test.\r",
            "This is an eighth test.",
            "This is a ninth test.\r",
            "This is a tenth test.", // note no newline at end
        ];

        let full_text = lines.join("\n");

        // calculate the known character offsets of each line
        let mut offsets = vec![];
        let mut start = 0;
        for (i, line) in lines.iter().enumerate() {
            // If it's not the last line, add 1 for the newline character.
            let end = start + line.len() + (i != (lines.len() - 1)) as usize;
            offsets.push((start, end));
            start = end;
        }

        // Test single-line extraction
        for i in 0..lines.len() - 1 {
            let span = LineSpan::new(i, i + 1);
            let expected = &full_text[offsets[i].0..offsets[i].1];
            let (extracted, start_offset, end_offset) = span.extract(&full_text).unwrap();

            assert_eq!(extracted, expected);
            assert_eq!((start_offset, end_offset), (offsets[i].0, offsets[i].1));
        }

        // Test multi-line extraction
        for i in 0..lines.len() {
            for j in i..=lines.len() {
                let span = LineSpan::new(i, j);

                assert!(span.line_count() == (j - i));

                if i == j {
                    assert!(span.is_empty());
                    continue;
                }

                let expected_start = offsets[i].0;
                let expected_end = offsets[j - 1].1;
                let expected_text = &full_text[expected_start..expected_end];

                let (extracted, start_offset, end_offset) = span.extract(&full_text).unwrap();
                assert_eq!(extracted, expected_text);
                assert_eq!((start_offset, end_offset), (expected_start, expected_end));
            }
        }
    }

    #[test]
    fn span_intersects() {
        let span = LineSpan::new(5, 11); // Exclusive end

        assert!(span.intersects(&LineSpan::new(10, 11))); // Intersect at start
        assert!(span.intersects(&LineSpan::new(0, 11))); // Fully contained
        assert!(span.intersects(&LineSpan::new(10, 15))); // Partial overlap
        assert!(span.intersects(&LineSpan::new(4, 6))); // Intersect at end
        assert!(span.intersects(&LineSpan::new(5, 6))); // Exact match start
        assert!(span.intersects(&LineSpan::new(0, 6))); // Overlap at end
        assert!(span.intersects(&LineSpan::new(0, 8))); // Overlap middle
        assert!(span.intersects(&LineSpan::new(0, 10))); // Overlap up to end
        assert!(span.intersects(&LineSpan::new(9, 10))); // Overlap at single point
        assert!(span.intersects(&LineSpan::new(7, 9))); // Overlap inside

        // Test cases where there should be no intersection due to exclusive end
        assert!(!span.intersects(&LineSpan::new(0, 5))); // Before start
        assert!(!span.intersects(&LineSpan::new(11, 20))); // After end
        assert!(!span.intersects(&LineSpan::new(11, 12))); // Just after end
    }
}
