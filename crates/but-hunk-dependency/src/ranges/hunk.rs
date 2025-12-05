use anyhow::{Context as _, Result, bail};
use but_core::{TreeStatusKind, ref_metadata::StackId};

use crate::utils::PaniclessSubtraction;

/// A struct for tracking what stack and commit a hunk belongs to as its line numbers shift with
/// new changes come in from other commits and/or stacks.
#[derive(Debug, Clone, PartialEq)]
pub struct HunkRange {
    /// The kind of change that was performed on the path of the parent-diff.
    pub change_type: TreeStatusKind,
    /// The stack that ownes `commit_id`.
    pub stack_id: StackId,
    /// The commit in the `stack_id`.
    pub commit_id: gix::ObjectId,
    /// The first line (1-based) at which this hunk is present.
    pub start: u32,
    /// The amount of lines the hunk is spanning.
    pub lines: u32,
    /// How many lines up or down this hunk moved when tracking it through the commits.
    pub line_shift: i32,
}

fn get_left_right(start: u32, lines: u32) -> Result<(i32, i32)> {
    let left = if lines == 0 { start } else { start - 1 };
    Ok((i32::try_from(left)?, i32::try_from(left + lines)?))
}

fn set_left_right(hunk_range: &mut HunkRange, left: i32, right: i32) -> Result<()> {
    hunk_range.lines = u32::try_from(right - left)?;
    hunk_range.start = u32::try_from(if hunk_range.lines == 0 {
        left
    } else {
        left + 1
    })?;
    Ok(())
}

pub(crate) struct ReceiveResult {
    pub left: Option<HunkRange>,
    pub incoming_line_shift_change: i32,
    pub right: Option<HunkRange>,
}

impl HunkRange {
    pub(crate) fn receive(
        mut self,
        incoming_start: u32,
        incoming_lines: u32,
    ) -> Result<ReceiveResult> {
        let (incoming_left, incoming_right) = get_left_right(incoming_start, incoming_lines)?;
        let (existing_left, existing_right) = get_left_right(self.start, self.lines)?;
        let existing_lines = self.lines;
        let existing_line_shift = self.line_shift;

        // Calculate if part or all of self will remain to the left of incoming
        // existing        L     R
        // incoming left 1-><-2-><-3
        let mut left_existing = if incoming_left <= existing_left {
            // 1. There will be no hunk to the left of incoming.
            None
        } else if incoming_left < existing_right {
            // 2. There will be a trimmed hunk to the left of incoming.
            let mut left_existing = self.clone();
            set_left_right(&mut left_existing, existing_left, incoming_left)?;
            Some(left_existing)
        } else {
            // 3. There will be an untrimmed hunk to the left of incoming.
            Some(self.clone())
        };

        // Calculate if part or all of self will remain to the right of incoming
        // existing         L     R
        // incoming right 3-><-2-><-1
        let mut right_existing = if incoming_right >= existing_right {
            // 1. There will be no hunk to the right of incoming.
            None
        } else if incoming_right > existing_left {
            // 2. There will be a trimmed hunk to the right of incoming.
            set_left_right(&mut self, incoming_right, existing_right)?;
            Some(self)
        } else {
            // 3. There will be an untrimmed hunk to the right of incoming.
            Some(self)
        };

        let incoming_line_shift_change = if let (None, None) = (&left_existing, &right_existing) {
            // No trace of existing hunk range, so attribute all its line shift
            // to the incoming hunk range.
            existing_line_shift
        } else {
            // Deduct trimmed lines from the `line_shift` of existing hunk(s) and
            // add them to the `line_shift` of incoming
            fn get_lines(hunk_range_option: &Option<HunkRange>) -> u32 {
                hunk_range_option
                    .as_ref()
                    .map_or(0, |hunk_range| hunk_range.lines)
            }
            let existing_lines_after_trimming =
                get_lines(&left_existing) + get_lines(&right_existing);
            let Some(trimmed_lines) = existing_lines.checked_sub(existing_lines_after_trimming)
            else {
                bail!("when calculating trimmed lines")
            };
            let trimmed_lines = i32::try_from(trimmed_lines)?;
            if let (Some(left_existing), Some(right_existing)) =
                (&mut left_existing, &mut right_existing)
            {
                // Here, both `left_existing` and `right_existing` will have the original
                // `line_shift` from `existing_hunk_range`.
                // Distribute proportionally according to their `lines`.
                let total_lines = i32::try_from(left_existing.lines + right_existing.lines)?;
                let total_net_line_shift = left_existing.line_shift - trimmed_lines;
                left_existing.line_shift = if total_lines == 0 {
                    // Arbitrarily split equally to avoid a division by zero.
                    total_net_line_shift / 2
                } else {
                    total_net_line_shift * i32::try_from(left_existing.lines)? / total_lines
                };
                right_existing.line_shift = total_net_line_shift - left_existing.line_shift;
            } else if let Some(left_existing) = &mut left_existing {
                left_existing.line_shift -= trimmed_lines;
            } else if let Some(right_existing) = &mut right_existing {
                right_existing.line_shift -= trimmed_lines;
            }
            trimmed_lines
        };
        Ok(ReceiveResult {
            left: left_existing,
            incoming_line_shift_change,
            right: right_existing,
        })
    }

    /// See if this range intersects with the hunk identified with `start` and `lines`.
    pub(crate) fn intersects(&self, start: u32, lines: u32) -> Result<bool> {
        if self.change_type == TreeStatusKind::Deletion {
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
}
