use crate::LineSpan;
use std::fmt;

pub mod memory;
#[cfg(feature = "mmap")]
pub mod mmap;

/// A file, delimited by lines. The line ending is unspecified;
/// it is assumed the underlying implementation handles (and omits)
/// line endings for us.
///
/// All text is assumed to be UTF-8.
pub trait LineFile<'a> {
    /// The type of iterator returned by [`LineFile::lines`] and [`LineFile::extract`].
    type LineIterator: Iterator<Item = &'a str>;

    /// Gets the line count of the file.
    fn line_count(&self) -> usize;

    /// Returns a slice of lines given the span.
    ///
    /// # Panics
    ///
    /// Panics if the span is out of bounds.
    fn extract(&'a self, span: LineSpan) -> Self::LineIterator;

    /// Returns an iterator over the all lines of the file.
    fn lines(&'a self) -> Self::LineIterator {
        self.extract(LineSpan::new(0, self.line_count() - 1))
    }

    /// Render the file to the given [`fmt::Write`]r.
    /// Will append each line, including the last, with the line ending
    /// specified by `line_endings`.
    fn render<W: fmt::Write>(
        &'a self,
        writer: &mut W,
        line_endings: LineEndings,
    ) -> Result<(), fmt::Error> {
        for line in self.lines() {
            writer.write_str(line)?;
            match line_endings {
                LineEndings::Unix => writer.write_char('\n')?,
                LineEndings::Windows => writer.write_str("\r\n")?,
            }
        }

        Ok(())
    }
}

/// The behavior of CRLF (carriage return + line feed) characters
/// when splitting a file into lines.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CrlfBehavior {
    /// Trims carriage returns (`\r`) from the end of lines.
    Trim,
    /// Keeps carriage returns (`\r`) at the end of lines.
    Keep,
}

/// Which line ending to emit when rendering files to a [`fmt::Write`]r.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LineEndings {
    /// Use Unix-style line endings (`\n`).
    Unix,
    /// Use Windows-style line endings (`\r\n`).
    Windows,
}
