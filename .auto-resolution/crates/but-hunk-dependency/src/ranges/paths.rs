use anyhow::bail;
use but_core::{TreeStatusKind, ref_metadata::StackId};

use crate::{HunkRange, InputDiffHunk, ranges::hunk::ReceiveResult};

/// Adds sequential diffs from sequential commits for a specific path, and shifts line numbers
/// with additions and deletions. It is expected that diffs are added one commit at a time,
/// each time merging the already added diffs with the new ones being added.
#[derive(Debug, Default)]
pub(crate) struct PathRanges {
    pub hunk_ranges: Vec<HunkRange>,
    commit_ids: Vec<gix::ObjectId>,
}

impl PathRanges {
    /// `change_type` is the kind of diff that the `incoming_hunks` was created from.
    pub fn add(
        &mut self,
        stack_id: StackId,
        commit_id: gix::ObjectId,
        change_type: TreeStatusKind,
        incoming_hunks: Vec<InputDiffHunk>,
    ) -> anyhow::Result<()> {
        if self.commit_ids.contains(&commit_id) {
            bail!("Commit ID already in stack: {}", commit_id)
        }

        let mut existing_hunk_ranges_iter =
            itertools::put_back(std::mem::take(&mut self.hunk_ranges));
        let mut line_shift: i32 = 0;

        for incoming_hunk in &incoming_hunks {
            let mut incoming_hunk_line_shift = incoming_hunk.net_lines()?;
            loop {
                let Some(existing_hunk_range) = existing_hunk_ranges_iter.next() else {
                    break;
                };
                let ReceiveResult {
                    above,
                    incoming_line_shift_change,
                    below,
                } = existing_hunk_range
                    .receive(incoming_hunk.old_start, incoming_hunk.old_lines)?;
                if let Some(mut above) = above {
                    above.start = {
                        let Some(start) = above.start.checked_add_signed(line_shift) else {
                            bail!("when calculating above.start")
                        };
                        start
                    };
                    self.hunk_ranges.push(above);
                }
                incoming_hunk_line_shift += incoming_line_shift_change;
                if let Some(below) = below {
                    if existing_hunk_ranges_iter.put_back(below).is_some() {
                        bail!("putting back too many");
                    };
                    break;
                }
            }
            self.hunk_ranges.push(HunkRange {
                change_type,
                stack_id,
                commit_id,
                start: incoming_hunk.new_start,
                lines: incoming_hunk.new_lines,
                line_shift: incoming_hunk_line_shift,
            });
            line_shift += incoming_hunk.net_lines()?;
        }
        for mut existing_hunk_range in existing_hunk_ranges_iter {
            existing_hunk_range.start = {
                let Some(start) = existing_hunk_range.start.checked_add_signed(line_shift) else {
                    bail!("when calculating existing_hunk_range.start")
                };
                start
            };
            self.hunk_ranges.push(existing_hunk_range);
        }

        Ok(())
    }
}
