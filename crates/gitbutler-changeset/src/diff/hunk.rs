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

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct TestHunk {
        removal_start: usize,
        addition_start: usize,
        changes: Vec<super::Change>,
    }

    impl super::RawHunk for TestHunk {
        type ChangeIterator = std::vec::IntoIter<super::Change>;

        fn get_removal_start(&self) -> usize {
            self.removal_start
        }

        fn get_addition_start(&self) -> usize {
            self.addition_start
        }

        fn changes(&self) -> Self::ChangeIterator {
            self.changes.clone().into_iter()
        }
    }

    impl fmt::Display for TestHunk {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            self.fmt_unified(f)
        }
    }

    #[test]
    fn empty_hunk() {
        let hunk = TestHunk {
            removal_start: 1,
            addition_start: 1,
            changes: vec![],
        };

        assert_eq!(format!("{hunk}"), "");
    }

    #[test]
    fn single_removal() {
        let hunk = TestHunk {
            removal_start: 30,
            addition_start: 38,
            changes: vec![super::Change::Removal("Hello, world!".to_string())],
        };

        assert_eq!(
            format!("{hunk}"),
            "@@ -30 +38,0 @@\n-Hello, world!\n\\ No newline at end of file\n"
        );
    }

    #[test]
    fn single_removal_trailing_nl() {
        let hunk = TestHunk {
            removal_start: 30,
            addition_start: 38,
            changes: vec![super::Change::Removal("Hello, world!\n".to_string())],
        };

        assert_eq!(format!("{hunk}"), "@@ -30 +38,0 @@\n-Hello, world!\n");
    }

    #[test]
    fn single_addition() {
        let hunk = TestHunk {
            removal_start: 30,
            addition_start: 38,
            changes: vec![super::Change::Addition("Hello, world!".to_string())],
        };

        assert_eq!(
            format!("{hunk}"),
            "@@ -30,0 +38 @@\n+Hello, world!\n\\ No newline at end of file\n"
        );
    }

    #[test]
    fn single_addition_trailing_nl() {
        let hunk = TestHunk {
            removal_start: 30,
            addition_start: 38,
            changes: vec![super::Change::Addition("Hello, world!\n".to_string())],
        };

        assert_eq!(format!("{hunk}"), "@@ -30,0 +38 @@\n+Hello, world!\n");
    }

    #[test]
    fn single_modified_line() {
        let hunk = TestHunk {
            removal_start: 30,
            addition_start: 38,
            changes: vec![
                super::Change::Removal("Hello, world!".to_string()),
                super::Change::Addition("Hello, GitButler!\n".to_string()),
            ],
        };

        assert_eq!(
            format!("{hunk}"),
            "@@ -30 +38 @@\n-Hello, world!\n\\ No newline at end of file\n+Hello, GitButler!\n"
        );
    }

    #[test]
    fn preserve_change_order() {
        let hunk = TestHunk {
            removal_start: 30,
            addition_start: 20,
            changes: vec![
                super::Change::Addition("Hello, GitButler!\n".to_string()),
                super::Change::Removal("Hello, world!\n".to_string()),
                super::Change::Removal("Hello, world 2!\n".to_string()),
                super::Change::Addition("Hello, GitButler 2!\n".to_string()),
                super::Change::Removal("Hello, world 3!".to_string()),
                super::Change::Addition("Hello, GitButler 3!\n".to_string()),
                super::Change::Addition("Hello, GitButler 4!".to_string()),
            ],
        };

        assert_eq!(
            format!("{hunk}"),
            "@@ -30,3 +20,4 @@\n+Hello, GitButler!\n-Hello, world!\n-Hello, world 2!\n+Hello, GitButler 2!\n-Hello, world 3!\n\\ No newline at end of file\n+Hello, GitButler 3!\n+Hello, GitButler 4!\n\\ No newline at end of file\n"
        );
    }
}
