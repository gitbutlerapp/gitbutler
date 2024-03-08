#![allow(clippy::module_name_repetitions)]

/// A line-based span of text.
///
/// All line spans are at least one line long.
/// Line numbers are 0-based.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct LineSpan {
    start: usize,
    end: usize,
}

impl LineSpan {
    /// Creates a new line span from the given start and end lines.
    /// Note that line numbers are zero-based, and the ending
    /// line number is inclusive.
    ///
    /// # Panics
    ///
    /// Panics if the start line is greater than the end line.
    #[must_use]
    pub fn new(start: usize, end: usize) -> Self {
        assert!(start <= end, "start line cannot be greater than end line");
        Self { start, end }
    }

    /// The starting line of the span. Zero-based.
    #[inline]
    #[must_use]
    pub fn start(&self) -> usize {
        self.start
    }

    /// The ending line of the span. Zero-based, inclusive.
    #[inline]
    #[must_use]
    pub fn end(&self) -> usize {
        self.end
    }

    /// Gets the line count from the span
    #[must_use]
    pub fn line_count(&self) -> usize {
        debug_assert!(self.end >= self.start);
        self.end - self.start + 1
    }

    /// Returns true if the given span intersects with this span.
    #[must_use]
    pub fn intersects(&self, other: &Self) -> bool {
        debug_assert!(self.end >= self.start);
        debug_assert!(other.end >= other.start);

        // If the other span starts after this span ends, they don't intersect.
        // If the other span ends before this span starts, they don't intersect.
        // Otherwise, they intersect.
        other.start <= self.end && other.end >= self.start
    }

    /// Extracts the lines from the span from the given text.
    /// The final line ending (if any) is not included.
    ///
    /// Also returns the character offsets (inclusive).
    ///
    /// # Panics
    /// Panics if the span's start > end.
    #[must_use]
    pub fn extract<'a>(&self, text: &'a str) -> Option<(&'a str, usize, usize)> {
        debug_assert!(self.end >= self.start);

        let mut start_offset = None;
        let mut current_line = 0;

        for (i, c) in text.char_indices() {
            if start_offset.is_none() && current_line == self.start {
                start_offset = Some(i);
            }

            if current_line == self.end {
                debug_assert!(start_offset.is_some());
                let start_offset = start_offset.unwrap();

                // Fast-forward to the end of the line and return
                // The strange song and dance is so that the final
                // line ending is not included, but we still gracefully
                // handle EOFs.
                let mut last_i = i;

                for (i, c) in text[i..].char_indices() {
                    if c == '\n' {
                        break;
                    }

                    last_i = i;
                }

                let end_offset = i + last_i;

                return Some((&text[start_offset..=end_offset], start_offset, end_offset));
            }

            if c == '\n' {
                current_line += 1;
            }
        }

        // Assert the invariant that we didn't mess up above.
        debug_assert!(current_line < self.end);

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn span_new() {
        for s in 0..20 {
            for e in 0..20 {
                if s > e {
                    assert!(std::panic::catch_unwind(|| LineSpan::new(s, e)).is_err());
                } else {
                    let span = LineSpan::new(s, e);
                    assert_eq!(span.start(), s);
                    assert_eq!(span.end(), e);
                }
            }
        }
    }

    #[test]
    fn span_extract() {
        let lines = vec![
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
            "This is a tenth test.",
        ];

        let full_text = lines.join("\n");

        // calculate the known character offsets of each line
        let lines = lines
            .into_iter()
            .scan(0, |state, line| {
                let start = *state;
                let end = start + line.len();
                *state = end + 1;
                Some((line, start, end - 1))
            })
            .collect::<Vec<_>>();

        // Test single-line extraction
        for (i, expected) in lines.iter().enumerate() {
            let span = LineSpan::new(i, i);
            let extracted = span.extract(&full_text).unwrap();
            assert_eq!(extracted, *expected);
        }

        // Test line span cartesian
        for (i, start_expected) in lines.iter().enumerate() {
            for (j, end_expected) in lines.iter().enumerate() {
                if i > j {
                    continue;
                }

                let expected = (
                    &full_text[start_expected.1..=end_expected.2],
                    start_expected.1,
                    end_expected.2,
                );

                let span = LineSpan::new(i, j);
                let extracted = span.extract(&full_text).unwrap();

                assert_eq!(extracted, expected);
            }
        }
    }

    #[test]
    fn span_intersects() {
        let span = LineSpan::new(5, 10);

        assert!(span.intersects(&LineSpan::new(10, 10)));
        assert!(span.intersects(&LineSpan::new(0, 10)));
        assert!(span.intersects(&LineSpan::new(10, 15)));
        assert!(span.intersects(&LineSpan::new(4, 5)));
        assert!(span.intersects(&LineSpan::new(5, 5)));
        assert!(span.intersects(&LineSpan::new(0, 5)));
        assert!(span.intersects(&LineSpan::new(0, 7)));
        assert!(span.intersects(&LineSpan::new(0, 9)));
        assert!(span.intersects(&LineSpan::new(9, 9)));
        assert!(span.intersects(&LineSpan::new(7, 8)));
        assert!(span.intersects(&LineSpan::new(5, 10)));

        assert!(!span.intersects(&LineSpan::new(0, 0)));
        assert!(!span.intersects(&LineSpan::new(0, 4)));
        assert!(!span.intersects(&LineSpan::new(4, 4)));
        assert!(!span.intersects(&LineSpan::new(11, 20)));
        assert!(!span.intersects(&LineSpan::new(11, 11)));
    }
}
