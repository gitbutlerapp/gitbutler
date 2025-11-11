use std::{
    collections::{HashMap, HashSet},
    vec,
};

use anyhow::{Context, bail};
use but_core::TreeStatusKind;
use but_core::ref_metadata::StackId;

use crate::{HunkRange, InputDiffHunk, utils::PaniclessSubtraction};

/// Adds sequential diffs from sequential commits for a specific path, and shifts line numbers
/// with additions and deletions. It is expected that diffs are added one commit at a time,
/// each time merging the already added diffs with the new ones being added.
#[derive(Debug, Default)]
pub(crate) struct PathRanges {
    pub hunk_ranges: Vec<HunkRange>,
    pub commit_dependencies: HashMap<gix::ObjectId, HashSet<gix::ObjectId>>,
    commit_ids: Vec<gix::ObjectId>,
    line_shift: i32,
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

        let mut index_next_hunk_to_visit: Option<usize> = None;
        let incoming_hunks_count = incoming_hunks.len();
        self.line_shift = 0;

        // This is the main loop that processes all diff hunks in a commit,
        // turning them into hunk ranges and inserting them in order.
        for incoming_hunk in &incoming_hunks {
            // Handle existing hunk range is a file deletion.
            if self.hunk_ranges.len() == 1
                && self.hunk_ranges[0].change_type == TreeStatusKind::Deletion
            {
                self.handle_file_recreation(
                    commit_id,
                    stack_id,
                    change_type,
                    incoming_hunks,
                    self.hunk_ranges[0],
                )?;
                break;
            }

            // Assume that an incoming hunk deleting a file is the only diff in the commit.
            if change_type == TreeStatusKind::Deletion {
                self.handle_file_deletion(
                    incoming_hunks_count,
                    change_type,
                    incoming_hunk,
                    stack_id,
                    commit_id,
                )?;
                break;
            }

            // If no existing hunk ranges, add all incoming hunks.
            if self.hunk_ranges.is_empty() {
                self.handle_add_all_hunks(stack_id, commit_id, change_type, incoming_hunks)?;
                break;
            }

            // Find all existing hunks that intersect with the incoming hunk.
            // --

            // If we already added a hunk, we need to check **only** the hunk ranges after that.
            // -> hunks are expected to be added in top to bottom and not overlapping.

            if let Some(i) = index_next_hunk_to_visit {
                // If the last hunk was added at the end, there are no more hunks to compare against.
                // -> we can just append the incoming hunk
                if i >= self.hunk_ranges.len() {
                    // Append the incoming hunk depends only of the commit that created the file (if any)
                    let file_creation_commit = self.find_file_creation_commit();

                    if let Some(file_creation_commit) = file_creation_commit {
                        self.track_commit_dependency(commit_id, vec![file_creation_commit])?;
                    }

                    if incoming_hunk.new_lines > 0 {
                        self.hunk_ranges.push(HunkRange {
                            change_type,
                            stack_id,
                            commit_id,
                            start: incoming_hunk.new_start,
                            lines: incoming_hunk.new_lines,
                            line_shift: incoming_hunk.net_lines()?,
                        });
                    }

                    index_next_hunk_to_visit = Some(self.hunk_ranges.len());
                    continue;
                }
            }

            // Start looking for intersecting hunks ranges after the last added hunk if there is one,
            // otherwise start from the beginning.
            let mut i = index_next_hunk_to_visit.unwrap_or_default();

            // Find all intersecting hunk ranges.
            let mut intersecting_hunks = vec![];
            while i < self.hunk_ranges.len() {
                let current_hunk = self.hunk_ranges[i];

                if current_hunk.lines == 0 {
                    i += 1;
                    continue;
                }

                // Current hunk range starts after the end of the incoming hunk.
                // -> we can stop looking for intersecting hunks
                if current_hunk.follows(
                    self.get_shifted_old_start(incoming_hunk.old_start),
                    incoming_hunk.old_lines,
                )? {
                    break;
                }

                // Current hunk range is ends before the start of the incoming hunk.
                if current_hunk.precedes(self.get_shifted_old_start(incoming_hunk.old_start))? {
                    i += 1;
                    continue;
                }

                if current_hunk.intersects(
                    self.get_shifted_old_start(incoming_hunk.old_start),
                    incoming_hunk.old_lines,
                )? {
                    intersecting_hunks.push((i, current_hunk));
                }

                i += 1;
            }

            // If there are no intersecting hunk ranges, we just add the incoming hunk.
            if intersecting_hunks.is_empty() {
                self.handle_no_intersecting_hunks(
                    commit_id,
                    i,
                    change_type,
                    incoming_hunk,
                    stack_id,
                    &mut index_next_hunk_to_visit,
                )?;
                continue;
            }

            // Handle multiple a single intersecting hunk.
            if intersecting_hunks.len() == 1 {
                self.handle_single_intersecting_hunk(
                    intersecting_hunks[0],
                    change_type,
                    incoming_hunk,
                    stack_id,
                    commit_id,
                    &mut index_next_hunk_to_visit,
                )?;
                continue;
            }

            self.handle_multiple_intersecting_hunks(
                intersecting_hunks,
                change_type,
                incoming_hunk,
                stack_id,
                commit_id,
                &mut index_next_hunk_to_visit,
            )?;
        }

        self.commit_ids.push(commit_id);

        Ok(())
    }

    fn handle_file_recreation(
        &mut self,
        commit_id: gix::ObjectId,
        stack_id: StackId,
        change_type: TreeStatusKind,
        incoming_hunks: Vec<InputDiffHunk>,
        existing_hunk_range: HunkRange,
    ) -> Result<(), anyhow::Error> {
        if incoming_hunks.len() > 1 {
            bail!("File recreation must be the only diff in a commit");
        }
        if change_type != TreeStatusKind::Addition {
            bail!("File recreation must be an addition");
        }

        self.track_commit_dependency(commit_id, vec![existing_hunk_range.commit_id])?;
        self.hunk_ranges.clear();
        self.handle_add_all_hunks(stack_id, commit_id, change_type, incoming_hunks)?;
        Ok(())
    }

    fn handle_file_deletion(
        &mut self,
        incoming_hunks_count: usize,
        change_type: TreeStatusKind,
        incoming_hunk: &InputDiffHunk,
        stack_id: StackId,
        commit_id: gix::ObjectId,
    ) -> Result<(), anyhow::Error> {
        // Incoming hunk is a file deletion.
        // This overrides all existing hunk ranges.
        if incoming_hunks_count > 1 {
            bail!("File deletion must be the only diff in a commit");
        }
        self.hunk_ranges = vec![HunkRange {
            change_type,
            stack_id,
            commit_id,
            start: incoming_hunk.new_start,
            lines: incoming_hunk.new_lines,
            line_shift: 0,
        }];

        // The commit that deletes a file depends on the last commit that touched it.
        if let Some(previous_commit_added) = self.commit_ids.last().copied() {
            self.track_commit_dependency(commit_id, vec![previous_commit_added])?;
        }

        Ok(())
    }

    /// Incoming hunk affects no hunk ranges.
    fn handle_no_intersecting_hunks(
        &mut self,
        commit_id: gix::ObjectId,
        index: usize,
        change_type: TreeStatusKind,
        incoming_hunk: &InputDiffHunk,
        stack_id: StackId,
        index_next_hunk_to_visit: &mut Option<usize>,
    ) -> Result<(), anyhow::Error> {
        // The incoming hunk does not intersect with anything.
        // The only commit that this depends on is the one that created the file.
        // That commit may or may not be available in the hunk list.
        let file_creation_commit = self.find_file_creation_commit();

        if let Some(file_creation_commit) = file_creation_commit {
            self.track_commit_dependency(commit_id, vec![file_creation_commit])?;
        }

        let (i_next_hunk_to_visit, i_first_hunk_to_shift) = self.insert_hunk_ranges_at(
            index,
            vec![HunkRange {
                change_type,
                stack_id,
                commit_id,
                start: incoming_hunk.new_start,
                lines: incoming_hunk.new_lines,
                line_shift: incoming_hunk.net_lines()?,
            }],
            0,
        );
        *index_next_hunk_to_visit = Some(i_next_hunk_to_visit);
        self.update_start_lines(i_first_hunk_to_shift, incoming_hunk.net_lines()?)?;
        Ok(())
    }

    /// Look for the commit that created the file.
    fn find_file_creation_commit(&mut self) -> Option<gix::ObjectId> {
        self.hunk_ranges
            .iter()
            .find(|h| h.change_type == TreeStatusKind::Addition)
            .map(|h| h.commit_id)
    }

    fn handle_add_all_hunks(
        &mut self,
        stack_id: StackId,
        commit_id: gix::ObjectId,
        change_type: TreeStatusKind,
        incoming_hunks: Vec<InputDiffHunk>,
    ) -> anyhow::Result<()> {
        for incoming_hunk in incoming_hunks {
            self.hunk_ranges.push(HunkRange {
                change_type,
                stack_id,
                commit_id,
                start: incoming_hunk.new_start,
                lines: incoming_hunk.new_lines,
                line_shift: incoming_hunk.net_lines()?,
            });
        }
        Ok(())
    }

    /// Incoming hunk affects multiple hunk ranges.
    fn handle_multiple_intersecting_hunks(
        &mut self,
        intersecting_hunk_ranges: Vec<(usize, HunkRange)>,
        change_type: TreeStatusKind,
        incoming_hunk: &InputDiffHunk,
        stack_id: StackId,
        commit_id: gix::ObjectId,
        index_next_hunk_to_visit: &mut Option<usize>,
    ) -> anyhow::Result<()> {
        // If there are multiple intersecting hunks, we can ignore all the intersecting hunk ranges
        // in the middle as they are considered to be completely overwritten by the incoming hunk.
        let net_lines = incoming_hunk.net_lines()?;
        let affected_commits = intersecting_hunk_ranges
            .iter()
            .map(|(_, hunk)| hunk.commit_id)
            .collect::<HashSet<_>>();

        // There are two possibilities:
        let (first_intersecting_hunk_index, first_intersecting_hunk) = intersecting_hunk_ranges
            .first()
            .ok_or(anyhow::anyhow!("No first intersecting hunk"))?;
        let (last_intersecting_hunk_index, last_intersecting_hunk) = intersecting_hunk_ranges
            .last()
            .ok_or(anyhow::anyhow!("No last intersecting hunk"))?;

        // 1. The incoming hunk completely overwrites the intersecting hunk ranges.
        if first_intersecting_hunk.covered_by(
            self.get_shifted_old_start(incoming_hunk.old_start),
            incoming_hunk.old_lines,
        ) && last_intersecting_hunk.covered_by(
            self.get_shifted_old_start(incoming_hunk.old_start),
            incoming_hunk.old_lines,
        ) {
            let (i_next_hunk_to_visit, i_first_hunk_to_shift) = self.replace_hunk_ranges_between(
                *first_intersecting_hunk_index,
                *last_intersecting_hunk_index + 1,
                vec![HunkRange {
                    change_type,
                    stack_id,
                    commit_id,
                    start: incoming_hunk.new_start,
                    lines: incoming_hunk.new_lines,
                    line_shift: net_lines,
                }],
                0,
            );

            self.track_commit_dependency(commit_id, affected_commits.into_iter().collect())?;
            *index_next_hunk_to_visit = Some(i_next_hunk_to_visit);
            self.update_start_lines(i_first_hunk_to_shift, net_lines)?;
            return Ok(());
        }

        // 2. The incoming hunk partially overwrites the intersecting hunk ranges.
        // 2.1. The incoming hunk overlaps the beginning of the second intersecting hunk range
        // -> we can tell because the first intersecting hunk range is completely covered by the incoming hunk.
        if first_intersecting_hunk.covered_by(
            self.get_shifted_old_start(incoming_hunk.old_start),
            incoming_hunk.old_lines,
        ) {
            let (i_next_hunk_to_visit, i_first_hunk_to_shift) = self.replace_hunk_ranges_between(
                *first_intersecting_hunk_index,
                *last_intersecting_hunk_index + 1,
                vec![
                    HunkRange {
                        change_type,
                        stack_id,
                        commit_id,
                        start: incoming_hunk.new_start,
                        lines: incoming_hunk.new_lines,
                        line_shift: net_lines,
                    },
                    HunkRange {
                        change_type: last_intersecting_hunk.change_type,
                        stack_id: last_intersecting_hunk.stack_id,
                        commit_id: last_intersecting_hunk.commit_id,
                        start: incoming_hunk.new_start + incoming_hunk.new_lines,
                        lines: self
                            .calculate_lines_of_trimmed_hunk(
                                last_intersecting_hunk,
                                change_type,
                                incoming_hunk,
                                "While calculating the lines of the bottom hunk range when incoming hunk overlaps the beginning of the second intersecting hunk range."

                            )?,
                        line_shift: last_intersecting_hunk.line_shift,
                    },
                ],
                0,
            );
            self.track_commit_dependency(commit_id, affected_commits.into_iter().collect())?;
            *index_next_hunk_to_visit = Some(i_next_hunk_to_visit);
            self.update_start_lines(i_first_hunk_to_shift, net_lines)?;
            return Ok(());
        }

        // 2.2. The incoming hunk overlaps the end of the first intersecting hunk range
        // -> we can tell because the last intersecting hunk range is completely covered by the incoming hunk.
        if last_intersecting_hunk.covered_by(
            self.get_shifted_old_start(incoming_hunk.old_start),
            incoming_hunk.old_lines,
        ) {
            let (i_next_hunk_to_visit, i_first_hunk_to_shift) = self.replace_hunk_ranges_between(
                *first_intersecting_hunk_index,
                *last_intersecting_hunk_index + 1,
                vec![
                    HunkRange {
                        change_type: first_intersecting_hunk.change_type,
                        stack_id: first_intersecting_hunk.stack_id,
                        commit_id: first_intersecting_hunk.commit_id,
                        start: first_intersecting_hunk.start,
                        lines: incoming_hunk
                            .new_start
                            .sub_or_err(first_intersecting_hunk.start)
                            .context("While calculating the lines when incoming hunk overlaps the end of the first intersecting hunk range.")?,
                        line_shift: first_intersecting_hunk.line_shift,
                    },
                    HunkRange {
                        change_type,
                        stack_id,
                        commit_id,
                        start: incoming_hunk.new_start,
                        lines: incoming_hunk.new_lines,
                        line_shift: net_lines,
                    },
                ],
                1,
            );
            self.track_commit_dependency(commit_id, affected_commits.into_iter().collect())?;
            *index_next_hunk_to_visit = Some(i_next_hunk_to_visit);
            self.update_start_lines(i_first_hunk_to_shift, net_lines)?;
            return Ok(());
        }

        // 2.3. The incoming hunk is contained in the intersecting hunk ranges
        let (i_next_hunk_to_visit, i_first_hunk_to_shift) = self.replace_hunk_ranges_between(
            *first_intersecting_hunk_index,
            *last_intersecting_hunk_index + 1,
            vec![
                HunkRange {
                    change_type: first_intersecting_hunk.change_type,
                    stack_id: first_intersecting_hunk.stack_id,
                    commit_id: first_intersecting_hunk.commit_id,
                    start: first_intersecting_hunk.start,
                    lines: incoming_hunk
                        .new_start
                        .sub_or_err(first_intersecting_hunk.start)
                        .context("While calculating the lines of the top hunk range when incoming hunk is contained in the intersecting hunk ranges.")?,
                    line_shift: first_intersecting_hunk.line_shift,
                },
                HunkRange {
                    change_type,
                    stack_id,
                    commit_id,
                    start: incoming_hunk.new_start,
                    lines: incoming_hunk.new_lines,
                    line_shift: net_lines,
                },
                HunkRange {
                    change_type: last_intersecting_hunk.change_type,
                    stack_id: last_intersecting_hunk.stack_id,
                    commit_id: last_intersecting_hunk.commit_id,
                    start: incoming_hunk.new_start + incoming_hunk.new_lines,
                    lines: self
                        .calculate_lines_of_trimmed_hunk(
                            last_intersecting_hunk,
                            change_type,
                            incoming_hunk,
                            "While calculating the lines of the bottom hunk range when incoming hunk is contained in the intersecting hunk ranges."
                        )?,
                    line_shift: last_intersecting_hunk.line_shift,
                },
            ],
            1,
        );
        self.track_commit_dependency(commit_id, affected_commits.into_iter().collect())?;
        *index_next_hunk_to_visit = Some(i_next_hunk_to_visit);
        self.update_start_lines(i_first_hunk_to_shift, net_lines)?;

        Ok(())
    }

    /// Incoming hunk only affects a single hunk range.
    fn handle_single_intersecting_hunk(
        &mut self,
        intersecting_hunk_range: (usize, HunkRange),
        change_type: TreeStatusKind,
        incoming_hunk: &InputDiffHunk,
        stack_id: StackId,
        commit_id: gix::ObjectId,
        index_next_hunk_to_visit: &mut Option<usize>,
    ) -> anyhow::Result<()> {
        // If there is only one intersecting hunk range there are three possibilities:
        let (index, hunk) = intersecting_hunk_range;
        let net_lines = incoming_hunk.net_lines()?;

        // 1. The incoming hunk completely overwrites the intersecting hunk.
        if hunk.covered_by(
            self.get_shifted_old_start(incoming_hunk.old_start),
            incoming_hunk.old_lines,
        ) {
            let (i_next_hunk_to_visit, i_first_hunk_to_shift) = self.replace_hunk_ranges_at(
                index,
                vec![HunkRange {
                    change_type,
                    stack_id,
                    commit_id,
                    start: incoming_hunk.new_start,
                    lines: incoming_hunk.new_lines,
                    line_shift: net_lines,
                }],
                0,
            );

            self.track_commit_dependency(commit_id, vec![hunk.commit_id])?;
            *index_next_hunk_to_visit = Some(i_next_hunk_to_visit);
            self.update_start_lines(i_first_hunk_to_shift, net_lines)?;
            return Ok(());
        }

        // 2. The incoming hunk is contained in the intersecting hunk range.
        if hunk.contains(
            self.get_shifted_old_start(incoming_hunk.old_start),
            incoming_hunk.old_lines,
        ) {
            let (i_next_hunk_to_visit, i_first_hunk_to_shift) = self.replace_hunk_ranges_at(
                index,
                vec![
                    HunkRange {
                        change_type: hunk.change_type,
                        stack_id: hunk.stack_id,
                        commit_id: hunk.commit_id,
                        start: hunk.start,
                        lines: incoming_hunk.new_start.sub_or_err(hunk.start).context(
                            "When calculating the top lines of the hunk range being split.",
                        )?,
                        line_shift: hunk.line_shift,
                    },
                    HunkRange {
                        change_type,
                        stack_id,
                        commit_id,
                        start: incoming_hunk.new_start,
                        lines: incoming_hunk.new_lines,
                        line_shift: net_lines,
                    },
                    HunkRange {
                        change_type: hunk.change_type,
                        stack_id: hunk.stack_id,
                        commit_id: hunk.commit_id,
                        start: incoming_hunk.new_start + incoming_hunk.new_lines,
                        lines: self.calculate_lines_of_trimmed_hunk(
                            &hunk,
                            change_type,
                            incoming_hunk,
                            "When calculating the bottom lines of the hunk range being split.",
                        )?,
                        line_shift: hunk.line_shift,
                    },
                ],
                1,
            );
            self.track_commit_dependency(commit_id, vec![hunk.commit_id])?;
            *index_next_hunk_to_visit = Some(i_next_hunk_to_visit);
            self.update_start_lines(i_first_hunk_to_shift, net_lines)?;
            return Ok(());
        }

        // 3. The incoming hunk partially overwrites the intersecting hunk range.
        let (i_next_hunk_to_visit, i_first_hunk_to_shift) = if self
            .get_shifted_old_start(incoming_hunk.old_start)
            <= hunk.start
        {
            // The incoming hunk overlaps the beginning of the intersecting hunk range.
            self.replace_hunk_ranges_at(
                index,
                vec![
                    HunkRange {
                        change_type,
                        stack_id,
                        commit_id,
                        start: incoming_hunk.new_start,
                        lines: incoming_hunk.new_lines,
                        line_shift: net_lines,
                    },
                    HunkRange {
                        change_type: hunk.change_type,
                        stack_id: hunk.stack_id,
                        commit_id: hunk.commit_id,
                        start: incoming_hunk.new_start + incoming_hunk.new_lines,
                        lines: self.calculate_lines_of_trimmed_hunk(
                            &hunk,
                            change_type,
                            incoming_hunk,
                            "When calculating the lines of the hunk range's beginning being trimmed.",
                        )?,
                        line_shift: net_lines,
                    },
                ],
                0,
            )
        } else {
            // The incoming hunk overlaps the end of the intersecting hunk range.
            self.replace_hunk_ranges_at(
                index,
                vec![
                    HunkRange {
                        change_type: hunk.change_type,
                        stack_id: hunk.stack_id,
                        commit_id: hunk.commit_id,
                        start: hunk.start,
                        lines: incoming_hunk.new_start.sub_or_err(hunk.start).context(
                            "When calculating the lines of the hunk range's end being trimmed.",
                        )?,
                        line_shift: hunk.line_shift,
                    },
                    HunkRange {
                        change_type,
                        stack_id,
                        commit_id,
                        start: incoming_hunk.new_start,
                        lines: incoming_hunk.new_lines,
                        line_shift: net_lines,
                    },
                ],
                1,
            )
        };

        self.track_commit_dependency(commit_id, vec![hunk.commit_id])?;
        *index_next_hunk_to_visit = Some(i_next_hunk_to_visit);
        self.update_start_lines(i_first_hunk_to_shift, net_lines)?;

        Ok(())
    }

    /// Calculate the number of lines of a hunk range that was trimmed from the top.
    ///
    /// Will handle the case where the incoming hunk is a modification and only adds or only deletes lines.
    fn calculate_lines_of_trimmed_hunk(
        &self,
        hunk: &HunkRange,
        change_type: TreeStatusKind,
        incoming_hunk: &InputDiffHunk,
        context: &'static str,
    ) -> anyhow::Result<u32> {
        let old_start = self.get_shifted_old_start(incoming_hunk.old_start);
        let addition_shift = if self.is_addition_only_hunk(change_type, incoming_hunk) {
            // If the incoming hunk is an addition, we need to subtract one more line.
            1
        } else {
            0
        };

        let deletion_shift = if self.is_deletion_only_hunk(change_type, incoming_hunk) {
            // If the incoming hunk is a deletion, we need to add one more line.
            1
        } else {
            0
        };

        let result = hunk.start + hunk.lines;
        let result = result.sub_or_err(old_start).context(context)?;
        let result = result
            .sub_or_err(incoming_hunk.old_lines)
            .context(context)?;
        let result = result.sub_or_err(addition_shift).context(context)?;
        Ok(result + deletion_shift)
    }

    /// Determine whether the incoming hunk is of modification type and only adds lines.
    fn is_addition_only_hunk(
        &self,
        change_type: TreeStatusKind,
        incoming_hunk: &InputDiffHunk,
    ) -> bool {
        let old_start = self.get_shifted_old_start(incoming_hunk.old_start);
        change_type == TreeStatusKind::Modification
            && (old_start + 1) == incoming_hunk.new_start
            && incoming_hunk.old_lines == 0
            && incoming_hunk.new_lines > 0
    }

    /// Determine whether the incoming hunk is of modification type and only deletes lines.
    fn is_deletion_only_hunk(
        &self,
        change_type: TreeStatusKind,
        incoming_hunk: &InputDiffHunk,
    ) -> bool {
        let old_start = self.get_shifted_old_start(incoming_hunk.old_start);
        change_type == TreeStatusKind::Modification
            && old_start == (incoming_hunk.new_start + 1)
            && incoming_hunk.old_lines > 0
            && incoming_hunk.new_lines == 0
    }

    fn track_commit_dependency(
        &mut self,
        commit_id: gix::ObjectId,
        parent_ids: Vec<gix::ObjectId>,
    ) -> anyhow::Result<()> {
        for parent_id in parent_ids {
            if commit_id == parent_id {
                bail!("Commit ID cannot be a parent ID");
            }
            self.commit_dependencies
                .entry(commit_id)
                .or_default()
                .insert(parent_id);
        }

        Ok(())
    }

    /// Shift the start lines of the hunk ranges starting at the given index.
    fn update_start_lines(
        &mut self,
        index_of_first_hunk: usize,
        line_shift: i32,
    ) -> anyhow::Result<()> {
        self.line_shift += line_shift;

        if index_of_first_hunk >= self.hunk_ranges.len() {
            return Ok(());
        }
        for hunk in &mut self.hunk_ranges[index_of_first_hunk..] {
            let new_start = hunk.start.checked_add_signed(line_shift).ok_or_else(|| {
                anyhow::anyhow!(
                    "Line shift overflow. Start: {} - Shift: {}",
                    hunk.start,
                    line_shift
                )
            })?;
            hunk.start = new_start;
        }
        Ok(())
    }

    /// Returns the shifted old start line number of an incoming hunk.
    fn get_shifted_old_start(&self, old_start: u32) -> u32 {
        // Everytime that we that an incoming hunk is added
        // and it adds or subtracts lines,
        // we need to shift the line numbers of the hunks that come after it.

        // This method allows us to compare the old start line number of the incoming hunk
        // with the shifted start line number of the existing hunk ranges.

        old_start.checked_add_signed(self.line_shift).unwrap_or(0)
    }

    /// Inserts the new hunks at the given index.
    ///
    /// Returns:
    /// - The index of the next hunk after the last added hunk.
    /// - The index of the next hunk after the hunk of interest.
    fn insert_hunk_ranges_at(
        &mut self,
        index: usize,
        hunks: Vec<HunkRange>,
        index_of_interest: usize,
    ) -> (usize, usize) {
        insert_hunk_ranges(
            &mut self.hunk_ranges,
            index,
            index,
            hunks,
            index_of_interest,
        )
    }

    /// Replaces the hunk at the given index with the new hunks.
    ///
    /// Returns:
    /// - The index of the next hunk after the last added hunk.
    /// - The index of the next hunk after the hunk of interest.
    fn replace_hunk_ranges_at(
        &mut self,
        index: usize,
        hunks: Vec<HunkRange>,
        index_of_interest: usize,
    ) -> (usize, usize) {
        insert_hunk_ranges(
            &mut self.hunk_ranges,
            index,
            index + 1,
            hunks,
            index_of_interest,
        )
    }

    /// Replaces the hunks between the given start and end indices with the new hunks.
    ///
    /// Returns:
    /// - The index of the next hunk after the last added hunk.
    /// - The index of the next hunk after the hunk of interest.
    fn replace_hunk_ranges_between(
        &mut self,
        start: usize,
        end: usize,
        hunks: Vec<HunkRange>,
        index_of_interest: usize,
    ) -> (usize, usize) {
        insert_hunk_ranges(&mut self.hunk_ranges, start, end, hunks, index_of_interest)
    }
}

/// Update the hunk ranges by inserting the new hunks at the given start and end indices.
///
/// Existing hunk ranges between the start and end indices are replaced by the new hunks.
/// Added hunk ranges that have 0 lines are ignored.
/// Returns:
/// - The index of the next hunk after the last added hunk.
/// - The index of the next hunk after the hunk of interest.
pub(crate) fn insert_hunk_ranges(
    hunk_ranges: &mut Vec<HunkRange>,
    start: usize,
    end: usize,
    hunks: Vec<HunkRange>,
    index_of_interest: usize,
) -> (usize, usize) {
    let mut new_hunks = vec![];
    new_hunks.extend_from_slice(&hunk_ranges[..start]);

    let mut index_after_last_added = start;
    let mut index_after_interest = start;
    for (i, hunk) in hunks.iter().enumerate() {
        // if hunk.lines > 0 {
        // }
        // Only add hunk ranges that have lines.
        new_hunks.push(*hunk);
        index_after_last_added += 1;

        if i == index_of_interest {
            index_after_interest = new_hunks.len();
        }
    }

    if end < hunk_ranges.len() {
        new_hunks.extend_from_slice(&hunk_ranges[end..]);
    }

    *hunk_ranges = new_hunks;

    (index_after_interest, index_after_last_added)
}
