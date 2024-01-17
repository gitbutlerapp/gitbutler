use crate::{CrlfBehavior, LineFile, LineSpan};

/// A [`LineFile`] stored in memory.
pub struct MemoryLineFile {
    lines: Vec<String>,
}

impl MemoryLineFile {
    /// Creates a new [`MemoryLineFile`] from the given lines.
    pub fn new(lines: Vec<String>) -> Self {
        Self { lines }
    }

    /// Creates a new [`MemoryLineFile`] from the given text,
    /// with the given CRLF behavior.
    pub fn from_str(text: &str, crlf_behavior: CrlfBehavior) -> Self {
        MemoryLineFile {
            lines: text
                .split('\n')
                .map(|line| match crlf_behavior {
                    CrlfBehavior::Trim => line.trim_end_matches('\r').to_owned(),
                    CrlfBehavior::Keep => line.to_owned(),
                })
                .collect(),
        }
    }
}

impl<'a> LineFile<'a> for MemoryLineFile {
    type LineIterator = impl Iterator<Item = &'a str>;

    #[inline]
    fn line_count(&self) -> usize {
        self.lines.len()
    }

    fn extract(&'a self, span: LineSpan) -> Self::LineIterator {
        self.lines[span.start()..=span.end()]
            .iter()
            .map(AsRef::as_ref)
    }
}
