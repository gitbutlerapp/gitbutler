use anyhow::{Context, Result};
use gitbutler_stack::StackId;

use crate::utils::PaniclessSubtraction;

/// A struct for tracking what stack and commit a hunk belongs to as its line numbers shift with
/// new changes come in from other commits and/or stacks.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HunkRange {
    pub change_type: gitbutler_diff::ChangeType,
    pub stack_id: StackId,
    pub commit_id: git2::Oid,
    pub start: u32,
    pub lines: u32,
    pub line_shift: i32,
}

impl HunkRange {
    pub fn intersects(&self, start: u32, lines: u32) -> Result<bool> {
        if self.change_type == gitbutler_diff::ChangeType::Deleted {
            // Special case when file is deleted.
            return Ok(true);
        }

        if start == 0 && lines == 0 {
            // Special case when adding lines at the top of the file.
            return Ok(false);
        }

        if lines == 0 {
            // Special case when only adding lines.
            return Ok(self.start <= start && self.start + self.lines > start);
        }

        let last_line = (self.start + self.lines)
            .sub_or_err(1)
            .context("While calculating the last line")?;

        let incoming_last_line = (start + lines)
            .sub_or_err(1)
            .context("While calculating the last line of the incoming hunk")?;

        if self.lines == 0 {
            // Special case when point is inside a range, happens
            // when a change contains only deletions.
            if self.line_shift < 0 {
                let lines_removed = 0 - self.line_shift;
                let this_start = i32::try_from(self.start)?;
                let last_line = (this_start + lines_removed) - 1;
                let incoming_start = i32::try_from(start)?;

                return Ok(self.start <= incoming_last_line && last_line >= incoming_start);
            }

            return Ok(self.start >= start && self.start < start + lines);
        }

        Ok(self.start <= incoming_last_line && last_line >= start)
    }

    pub fn contains(&self, start: u32, lines: u32) -> bool {
        if lines == 0 {
            // Special case when only adding lines.
            return self.start <= start && self.start + self.lines > start + 1;
        }
        start > self.start && start + lines <= self.start + self.lines
    }

    pub fn covered_by(&self, start: u32, lines: u32) -> bool {
        if start == 0 && lines == 0 {
            // Special when adding lines at the top of the file.
            return false;
        }
        self.start >= start && self.start + self.lines <= start + lines
    }

    pub fn precedes(&self, start: u32) -> Result<bool> {
        let last_line = (self.start + self.lines)
            .sub_or_err(1)
            .context("While calculating the last line")?;

        Ok(last_line < start)
    }

    pub fn follows(&self, start: u32, lines: u32) -> Result<bool> {
        if start == 0 && lines == 0 {
            // Special case when adding lines at the top of the file.
            return Ok(true);
        }

        if lines == 0 {
            // Special case when only adding lines.
            return Ok(self.start > start);
        }

        let incoming_last_line = (start + lines)
            .sub_or_err(1)
            .context("While calculating the last line of the incoming hunk")?;

        Ok(self.start > incoming_last_line)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_deleted_file_intersects_everything() {
        let range = HunkRange {
            change_type: gitbutler_diff::ChangeType::Deleted,
            stack_id: StackId::generate(),
            commit_id: git2::Oid::from_str("a").unwrap(),
            start: 0,
            lines: 0,
            line_shift: 0,
        };

        assert!(range.intersects(1, 1).unwrap());
        assert!(range.intersects(2, 2).unwrap());
        assert!(range.intersects(1, 1).unwrap());
        assert!(range.intersects(12, 10).unwrap());
        assert!(range.intersects(4, 0).unwrap());
        assert!(range.intersects(0, 0).unwrap());
    }

    #[test]
    fn test_hunk_at_the_beginning() {
        let range = HunkRange {
            change_type: gitbutler_diff::ChangeType::Modified,
            stack_id: StackId::generate(),
            commit_id: git2::Oid::from_str("a").unwrap(),
            start: 1,
            lines: 10,
            line_shift: 10,
        };

        assert!(range.intersects(1, 1).unwrap());
        assert!(range.intersects(1, 10).unwrap());
        assert!(range.intersects(4, 2).unwrap());
        assert!(range.intersects(10, 20).unwrap());
        assert!(range.intersects(4, 0).unwrap());
        // Adding lines at the beginning of the file.
        assert!(!range.intersects(0, 0).unwrap());

        assert!(!range.intersects(11, 20).unwrap());
        assert!(!range.intersects(30, 1).unwrap());
    }

    #[test]
    fn test_hunk_in_the_middle() {
        let range = HunkRange {
            change_type: gitbutler_diff::ChangeType::Modified,
            stack_id: StackId::generate(),
            commit_id: git2::Oid::from_str("a").unwrap(),
            start: 10,
            lines: 10,
            line_shift: 0,
        };

        assert!(range.intersects(1, 10).unwrap());
        assert!(range.intersects(1, 20).unwrap());
        assert!(range.intersects(1, 30).unwrap());
        assert!(range.intersects(4, 10).unwrap());
        assert!(range.intersects(19, 0).unwrap());
        assert!(range.intersects(10, 0).unwrap());
        assert!(range.intersects(10, 10).unwrap());
        assert!(range.intersects(10, 20).unwrap());
        assert!(range.intersects(11, 20).unwrap());
        assert!(range.intersects(15, 1).unwrap());

        // Adding lines at the beginning of the file.
        assert!(!range.intersects(0, 0).unwrap());

        assert!(!range.intersects(20, 0).unwrap());
        assert!(!range.intersects(1, 1).unwrap());
        assert!(!range.intersects(1, 9).unwrap());
        assert!(!range.intersects(20, 1).unwrap());
        assert!(!range.intersects(30, 1).unwrap());
    }

    #[test]
    fn test_is_covered_by() {
        let range = HunkRange {
            change_type: gitbutler_diff::ChangeType::Modified,
            stack_id: StackId::generate(),
            commit_id: git2::Oid::from_str("a").unwrap(),
            start: 10,
            lines: 10,
            line_shift: 0,
        };

        assert!(range.covered_by(1, 20));
        assert!(range.covered_by(1, 30));
        assert!(range.covered_by(4, 16));
        assert!(range.covered_by(10, 20));
        // Adding lines at the beginning of the file.
        assert!(!range.covered_by(0, 0));

        assert!(!range.covered_by(10, 9));
        assert!(!range.covered_by(11, 20));
        assert!(!range.covered_by(15, 1));
        assert!(!range.covered_by(1, 1));
        assert!(!range.covered_by(1, 18));
        assert!(!range.covered_by(20, 1));
        assert!(!range.covered_by(30, 10));
    }

    #[test]
    fn test_contains() {
        let range = HunkRange {
            change_type: gitbutler_diff::ChangeType::Modified,
            stack_id: StackId::generate(),
            commit_id: git2::Oid::from_str("a").unwrap(),
            start: 10,
            lines: 10,
            line_shift: 0,
        };

        assert!(!range.contains(0, 0));
        assert!(!range.contains(1, 20));
        assert!(!range.contains(1, 30));
        assert!(!range.contains(4, 16));
        assert!(!range.contains(10, 20));
        assert!(!range.contains(10, 10));
        assert!(!range.contains(19, 0));
        assert!(range.contains(11, 8));
        assert!(range.contains(11, 9));
        assert!(range.contains(10, 0));
        assert!(range.contains(18, 0));
    }

    #[test]
    fn test_follows() {
        let range = HunkRange {
            change_type: gitbutler_diff::ChangeType::Modified,
            stack_id: StackId::generate(),
            commit_id: git2::Oid::from_str("a").unwrap(),
            start: 10,
            lines: 10,
            line_shift: 0,
        };

        assert!(range.follows(0, 0).unwrap());
        assert!(range.follows(1, 9).unwrap());
        assert!(range.follows(9, 1).unwrap());
        assert!(!range.follows(10, 0).unwrap());
        assert!(!range.follows(11, 0).unwrap());
        assert!(!range.follows(10, 1).unwrap());
        assert!(!range.follows(11, 1).unwrap());
        assert!(!range.follows(20, 1).unwrap());
    }
}
