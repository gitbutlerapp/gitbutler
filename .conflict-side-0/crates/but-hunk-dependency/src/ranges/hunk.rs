use anyhow::{Context as _, Result, bail};
use but_core::{TreeStatusKind, ref_metadata::StackId};

use crate::utils::PaniclessSubtraction;

/// A struct for tracking what stack and commit a hunk belongs to as its line numbers shift with
/// new changes come in from other commits and/or stacks.
#[derive(Debug, Clone, PartialEq)]
pub struct HunkRange {
    /// The kind of change that was performed on the path of the parent-diff.
    pub change_type: TreeStatusKind,
    /// The stack that owns `commit_id`.
    pub stack_id: StackId,
    /// The commit in the `stack_id`.
    pub commit_id: gix::ObjectId,
    /// The first line (1-based) at which this hunk is present. If `lines == 0`,
    /// represents the space immediately after the given line, like in unified
    /// diff format (0 is the start of the file, 1 is immediately after line 1
    /// and before line 2, and so on).
    pub start: u32,
    /// The amount of lines the hunk is spanning.
    pub lines: u32,
    /// How many lines up or down this hunk moved when tracking it through the commits.
    pub line_shift: i32,
}

/// Gets the top and bottom edges of the given hunk. `top` is 0 if the hunk
/// starts at the start of the file, 1 if it starts immediately below the first
/// line, and so on. Likewise for `bottom`.
fn get_top_bottom(start: u32, lines: u32) -> Result<(i32, i32)> {
    let top = if lines == 0 { start } else { start - 1 };
    Ok((i32::try_from(top)?, i32::try_from(top + lines)?))
}

/// Inverse of [get_top_bottom()].
fn set_top_bottom(hunk_range: &mut HunkRange, top: i32, bottom: i32) -> Result<()> {
    hunk_range.lines = u32::try_from(bottom - top)?;
    hunk_range.start = u32::try_from(if hunk_range.lines == 0 { top } else { top + 1 })?;
    Ok(())
}

pub(crate) struct ReceiveResult {
    /// What's remaining of the receiver [HunkRange] above the incoming hunk,
    /// if anything.
    pub above: Option<HunkRange>,
    /// How much the incoming hunk's line shift needs to change due to it
    /// partially or completely consuming other hunks.
    pub incoming_line_shift_change: i32,
    /// What's remaining of the receiver [HunkRange] below the incoming hunk,
    /// if anything.
    pub below: Option<HunkRange>,
}

impl HunkRange {
    pub(crate) fn receive(
        mut self,
        incoming_start: u32,
        incoming_lines: u32,
    ) -> Result<ReceiveResult> {
        let (incoming_top, incoming_bottom) = get_top_bottom(incoming_start, incoming_lines)?;
        let (self_top, self_bottom) = get_top_bottom(self.start, self.lines)?;
        let self_lines = self.lines;
        let self_line_shift = self.line_shift;

        // Calculate if part or all of self will remain above incoming
        let mut above = if incoming_top <= self_top {
            // There will be no self above incoming.
            None
        } else if incoming_top < self_bottom {
            // There will be a trimmed self above incoming.
            let mut above = self.clone();
            set_top_bottom(&mut above, self_top, incoming_top)?;
            Some(above)
        } else {
            // There will be an untrimmed hunk above incoming.
            Some(self.clone())
        };

        // Calculate if part or all of self will remain below incoming
        let mut below = if incoming_bottom >= self_bottom {
            // There will be no self below incoming.
            None
        } else if incoming_bottom > self_top {
            // There will be a trimmed self below incoming.
            set_top_bottom(&mut self, incoming_bottom, self_bottom)?;
            Some(self)
        } else {
            // There will be an untrimmed self below incoming.
            Some(self)
        };

        let incoming_line_shift_change = if let (None, None) = (&above, &below) {
            // No trace of self. Its line shift must go somewhere - put it all
            // in incoming.
            self_line_shift
        } else {
            // Calculate how many lines to deduct from self's line shift (and
            // add to incoming's line shift).
            fn get_lines(hunk_range_option: &Option<HunkRange>) -> u32 {
                hunk_range_option
                    .as_ref()
                    .map_or(0, |hunk_range| hunk_range.lines)
            }
            let self_lines_after_trimming = get_lines(&above) + get_lines(&below);
            let Some(lines_to_deduct) = self_lines.checked_sub(self_lines_after_trimming) else {
                bail!("when calculating trimmed lines")
            };
            let lines_to_deduct = i32::try_from(lines_to_deduct)?;

            // Deduct and add.
            if let (Some(above), Some(below)) = (&mut above, &mut below) {
                // self has split in two (possible if incoming's top is below
                // self's top and incoming's bottom is above self's bottom).
                // Recalculate both line shifts from scratch.
                let total_lines = i32::try_from(above.lines + below.lines)?;
                let total_net_line_shift = self_line_shift - lines_to_deduct;
                above.line_shift = if total_lines == 0 {
                    // Arbitrarily split equally to avoid a division by zero.
                    total_net_line_shift / 2
                } else {
                    total_net_line_shift * i32::try_from(above.lines)? / total_lines
                };
                below.line_shift = total_net_line_shift - above.line_shift;
            } else if let Some(above) = &mut above {
                above.line_shift -= lines_to_deduct;
            } else if let Some(below) = &mut below {
                below.line_shift -= lines_to_deduct;
            }
            lines_to_deduct
        };
        Ok(ReceiveResult {
            above,
            incoming_line_shift_change,
            below,
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
