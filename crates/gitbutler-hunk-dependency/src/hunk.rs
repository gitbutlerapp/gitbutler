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
