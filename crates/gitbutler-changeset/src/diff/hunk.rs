use core::fmt;

/// A single line change in a diff. Note that
/// hunks MUST NOT have context lines.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Change {
    /// The line was added.
    ///
    /// **Do not include the leading '+' character**,
    /// and make sure that **the final (CR)LF _is_ included**, if any.
    /// The only case where a newline can be omitted is at the end of the file.
    Addition(String),
    /// The line was removed.
    ///
    /// **Do not include the leading '-' character**,
    /// and make sure that **the final (CR)LF _is_ included**, if any.
    /// The only case where a newline can be omitted is at the end of the file.
    Removal(String),
}

/// Represents a "raw" hunk of a file diff.
/// "Raw" hunks are hunks that are reported directly
/// from Git or Git-like sources, whereby the hunk
/// represents a single, contiguous set of changes
/// that have not been split up into adjacent hunks
/// or the like.
pub trait RawHunk {
    /// The type of iterator that produces [`Change`]s.
    type ChangeIterator: Iterator<Item = Change> + Clone + 'static;

    /// Returns the line at which removals begin.
    /// Note that lines start at 1.
    fn get_removal_start(&self) -> usize;
    /// Returns the line at which additions begin.
    /// Note that lines start at 1.
    fn get_addition_start(&self) -> usize;
    /// Returns an iterator over the additions and removals
    /// in the hunk.
    fn changes(&self) -> Self::ChangeIterator;
}

/// Formats the hunk as a unified diff.
pub trait FormatHunk: RawHunk {
    /// Formats the hunk as a unified diff.
    fn fmt_unified(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let removal_start = self.get_removal_start();
        let addition_start = self.get_addition_start();

        let mut removal_count = 0;
        let mut addition_count = 0;

        let mut last_addition_index = 0;
        let mut last_removal_index = 0;

        for (i, change) in self.changes().enumerate() {
            match change {
                Change::Addition(_) => {
                    addition_count += 1;
                    last_addition_index = i;
                }
                Change::Removal(_) => {
                    removal_count += 1;
                    last_removal_index = i;
                }
            }
        }

        if removal_count == 0 && addition_count == 0 {
            return Ok(());
        }

        write!(f, "@@ -{removal_start}")?;
        if removal_count != 1 {
            write!(f, ",{removal_count}")?;
        }
        write!(f, " +{addition_start}")?;
        if addition_count != 1 {
            write!(f, ",{addition_count}")?;
        }
        writeln!(f, " @@")?;

        for (i, change) in self.changes().enumerate() {
            let line = match change {
                Change::Addition(line) => {
                    write!(f, "+{}", line)?;
                    line
                }
                Change::Removal(line) => {
                    write!(f, "-{}", line)?;
                    line
                }
            };

            if (i == last_removal_index || i == last_addition_index) && !line.ends_with('\n') {
                write!(f, "\n\\ No newline at end of file\n")?;
            }
        }

        Ok(())
    }
}

impl<T> FormatHunk for T where T: RawHunk {}
