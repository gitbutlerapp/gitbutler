use std::{
    collections::{HashMap, HashSet},
    vec,
};

use anyhow::bail;
use gitbutler_stack::StackId;

use crate::{HunkRange, InputDiff};

/// Adds sequential diffs from sequential commits for a specific path, and shifts line numbers
/// with additions and deletions. It is expected that diffs are added one commit at a time,
/// each time merging the already added diffs with the new ones being added.
#[derive(Debug, Default)]
pub struct PathRanges {
    pub hunk_ranges: Vec<HunkRange>,
    pub commit_dependencies: HashMap<git2::Oid, HashSet<git2::Oid>>,
    commit_ids: Vec<git2::Oid>,
    line_shift: i32,
}

impl PathRanges {
    pub fn add(
        &mut self,
        stack_id: StackId,
        commit_id: git2::Oid,
        incoming_hunks: Vec<InputDiff>,
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
                && self.hunk_ranges[0].change_type == gitbutler_diff::ChangeType::Deleted
            {
                self.handle_file_recreation(
                    commit_id,
                    stack_id,
                    incoming_hunks,
                    self.hunk_ranges[0],
                )?;
                break;
            }

            // Assume that an incoming hunk deleting a file is the only diff in the commit.
            if incoming_hunk.change_type == gitbutler_diff::ChangeType::Deleted {
                self.handle_file_deletion(
                    incoming_hunks_count,
                    incoming_hunk,
                    stack_id,
                    commit_id,
                )?;
                break;
            }

            // If no existing hunk ranges, add all incoming hunks.
            if self.hunk_ranges.is_empty() {
                self.handle_add_all_hunks(stack_id, commit_id, incoming_hunks)?;
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
                            change_type: incoming_hunk.change_type,
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

                // Current hunk range starts after the end of the incoming hunk.
                // -> we can stop looking for intersecting hunks
                if current_hunk.follows(
                    self.get_shifted_old_start(incoming_hunk.old_start),
                    incoming_hunk.old_lines,
                ) {
                    break;
                }

                // Current hunk range is ends before the start of the incoming hunk.
                if current_hunk.precedes(self.get_shifted_old_start(incoming_hunk.old_start)) {
                    i += 1;
                    continue;
                }

                if current_hunk.intersects(
                    self.get_shifted_old_start(incoming_hunk.old_start),
                    incoming_hunk.old_lines,
                ) {
                    intersecting_hunks.push((i, current_hunk));
                }

                i += 1;
            }

            // If there are no intersecting hunk ranges, we just add the incoming hunk.
            if intersecting_hunks.is_empty() {
                self.handle_no_intersecting_hunks(
                    commit_id,
                    i,
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
                    incoming_hunk,
                    stack_id,
                    commit_id,
                    &mut index_next_hunk_to_visit,
                )?;
                continue;
            }

            self.handle_multiple_intersecting_hunks(
                intersecting_hunks,
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
        commit_id: git2::Oid,
        stack_id: gitbutler_id::id::Id<gitbutler_stack::Stack>,
        incoming_hunks: Vec<InputDiff>,
        existing_hunk_range: HunkRange,
    ) -> Result<(), anyhow::Error> {
        if incoming_hunks.len() > 1 {
            bail!("File recreation must be the only diff in a commit");
        }
        let incoming_hunk = &incoming_hunks[0];
        if incoming_hunk.change_type != gitbutler_diff::ChangeType::Added {
            bail!("File recreation must be an addition");
        }

        self.track_commit_dependency(commit_id, vec![existing_hunk_range.commit_id])?;
        self.hunk_ranges.clear();
        self.handle_add_all_hunks(stack_id, commit_id, incoming_hunks)?;
        Ok(())
    }

    fn handle_file_deletion(
        &mut self,
        incoming_hunks_count: usize,
        incoming_hunk: &InputDiff,
        stack_id: gitbutler_id::id::Id<gitbutler_stack::Stack>,
        commit_id: git2::Oid,
    ) -> Result<(), anyhow::Error> {
        // Incoming hunk is a file deletion.
        // This overrides all existing hunk ranges.
        if incoming_hunks_count > 1 {
            bail!("File deletion must be the only diff in a commit");
        }
        self.hunk_ranges = vec![HunkRange {
            change_type: incoming_hunk.change_type,
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
        commit_id: git2::Oid,
        index: usize,
        incoming_hunk: &InputDiff,
        stack_id: gitbutler_id::id::Id<gitbutler_stack::Stack>,
        index_next_hunk_to_visit: &mut Option<usize>,
    ) -> Result<(), anyhow::Error> {
        // The incoming hunk does not intersect with anything.
        // The only commit that this depends on is the one that created the file.
        // That commit may or may not be available in the hunk list.
        let file_creation_commit = self.find_file_creation_commit();

        if let Some(file_creation_commit) = file_creation_commit {
            self.track_commit_dependency(commit_id, vec![file_creation_commit])?;
        }

        let index_of_next = self.insert_hunk_ranges_at(
            index,
            vec![HunkRange {
                change_type: incoming_hunk.change_type,
                stack_id,
                commit_id,
                start: incoming_hunk.new_start,
                lines: incoming_hunk.new_lines,
                line_shift: incoming_hunk.net_lines()?,
            }],
        );
        *index_next_hunk_to_visit = Some(index_of_next);
        self.update_start_lines(index_of_next, incoming_hunk.net_lines()?)?;
        Ok(())
    }

    /// Look for the commit that created the file.
    fn find_file_creation_commit(&mut self) -> Option<git2::Oid> {
        let file_creation_commit = self
            .hunk_ranges
            .iter()
            .find(|h| h.change_type == gitbutler_diff::ChangeType::Added)
            .map(|h| h.commit_id);
        file_creation_commit
    }

    fn handle_add_all_hunks(
        &mut self,
        stack_id: gitbutler_id::id::Id<gitbutler_stack::Stack>,
        commit_id: git2::Oid,
        incoming_hunks: Vec<InputDiff>,
    ) -> anyhow::Result<()> {
        for hunk in incoming_hunks {
            self.hunk_ranges.push(HunkRange {
                change_type: hunk.change_type,
                stack_id,
                commit_id,
                start: hunk.new_start,
                lines: hunk.new_lines,
                line_shift: hunk.net_lines()?,
            });
        }
        Ok(())
    }

    /// Incoming hunk affects multiple hunk ranges.
    fn handle_multiple_intersecting_hunks(
        &mut self,
        intersecting_hunk_ranges: Vec<(usize, HunkRange)>,
        incoming_hunk: &InputDiff,
        stack_id: gitbutler_id::id::Id<gitbutler_stack::Stack>,
        commit_id: git2::Oid,
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
            let index_of_next = self.replace_hunk_ranges_between(
                *first_intersecting_hunk_index,
                *last_intersecting_hunk_index + 1,
                vec![HunkRange {
                    change_type: incoming_hunk.change_type,
                    stack_id,
                    commit_id,
                    start: incoming_hunk.new_start,
                    lines: incoming_hunk.new_lines,
                    line_shift: net_lines,
                }],
            );

            self.track_commit_dependency(commit_id, affected_commits.into_iter().collect())?;
            *index_next_hunk_to_visit = Some(index_of_next);
            self.update_start_lines(index_of_next, net_lines)?;
            return Ok(());
        }

        // 2. The incoming hunk partially overwrites the intersecting hunk ranges.
        // 2.1. The incoming hunk overlaps the beginning of the second intersecting hunk range
        // -> we can tell because the first intersecting hunk range is completely covered by the incoming hunk.
        if first_intersecting_hunk.covered_by(
            self.get_shifted_old_start(incoming_hunk.old_start),
            incoming_hunk.old_lines,
        ) {
            let index_of_next = self.replace_hunk_ranges_between(
                *first_intersecting_hunk_index,
                *last_intersecting_hunk_index + 1,
                vec![
                    HunkRange {
                        change_type: incoming_hunk.change_type,
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
                        lines: last_intersecting_hunk.start + last_intersecting_hunk.lines
                            - self.get_shifted_old_start(incoming_hunk.old_start)
                            - incoming_hunk.old_lines,
                        line_shift: last_intersecting_hunk.line_shift,
                    },
                ],
            );
            self.track_commit_dependency(commit_id, affected_commits.into_iter().collect())?;
            *index_next_hunk_to_visit = Some(index_of_next);
            self.update_start_lines(index_of_next, net_lines)?;
            return Ok(());
        }

        // 2.2. The incoming hunk overlaps the end of the first intersecting hunk range
        // -> we can tell because the last intersecting hunk range is completely covered by the incoming hunk.
        if last_intersecting_hunk.covered_by(
            self.get_shifted_old_start(incoming_hunk.old_start),
            incoming_hunk.old_lines,
        ) {
            let index_of_next = self.replace_hunk_ranges_between(
                *first_intersecting_hunk_index,
                *last_intersecting_hunk_index + 1,
                vec![
                    HunkRange {
                        change_type: first_intersecting_hunk.change_type,
                        stack_id: first_intersecting_hunk.stack_id,
                        commit_id: first_intersecting_hunk.commit_id,
                        start: first_intersecting_hunk.start,
                        lines: incoming_hunk.new_start - first_intersecting_hunk.start,
                        line_shift: first_intersecting_hunk.line_shift,
                    },
                    HunkRange {
                        change_type: incoming_hunk.change_type,
                        stack_id,
                        commit_id,
                        start: incoming_hunk.new_start,
                        lines: incoming_hunk.new_lines,
                        line_shift: net_lines,
                    },
                ],
            );
            self.track_commit_dependency(commit_id, affected_commits.into_iter().collect())?;
            *index_next_hunk_to_visit = Some(index_of_next);
            self.update_start_lines(index_of_next, net_lines)?;
            return Ok(());
        }

        // 2.3. The incoming hunk is contained in the intersecting hunk ranges
        let index_of_next = self.replace_hunk_ranges_between(
            *first_intersecting_hunk_index,
            *last_intersecting_hunk_index + 1,
            vec![
                HunkRange {
                    change_type: first_intersecting_hunk.change_type,
                    stack_id: first_intersecting_hunk.stack_id,
                    commit_id: first_intersecting_hunk.commit_id,
                    start: first_intersecting_hunk.start,
                    lines: incoming_hunk.new_start - first_intersecting_hunk.start,
                    line_shift: first_intersecting_hunk.line_shift,
                },
                HunkRange {
                    change_type: incoming_hunk.change_type,
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
                    lines: last_intersecting_hunk.start + last_intersecting_hunk.lines
                        - self.get_shifted_old_start(incoming_hunk.old_start)
                        - incoming_hunk.old_lines,
                    line_shift: last_intersecting_hunk.line_shift,
                },
            ],
        );
        self.track_commit_dependency(commit_id, affected_commits.into_iter().collect())?;
        *index_next_hunk_to_visit = Some(index_of_next);
        self.update_start_lines(index_of_next, net_lines)?;

        Ok(())
    }

    /// Incoming hunk only affects a single hunk range.
    fn handle_single_intersecting_hunk(
        &mut self,
        intersecting_hunk_range: (usize, HunkRange),
        incoming_hunk: &InputDiff,
        stack_id: gitbutler_id::id::Id<gitbutler_stack::Stack>,
        commit_id: git2::Oid,
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
            let index_of_next = self.replace_hunk_ranges_at(
                index,
                vec![HunkRange {
                    change_type: incoming_hunk.change_type,
                    stack_id,
                    commit_id,
                    start: incoming_hunk.new_start,
                    lines: incoming_hunk.new_lines,
                    line_shift: net_lines,
                }],
            );

            self.track_commit_dependency(commit_id, vec![hunk.commit_id])?;
            *index_next_hunk_to_visit = Some(index_of_next);
            self.update_start_lines(index_of_next, net_lines)?;
            return Ok(());
        }

        // 2. The incoming hunk is contained in the intersecting hunk range.
        if hunk.contains(
            self.get_shifted_old_start(incoming_hunk.old_start),
            incoming_hunk.old_lines,
        ) {
            let index_of_next = self.replace_hunk_ranges_at(
                index,
                vec![
                    HunkRange {
                        change_type: hunk.change_type,
                        stack_id: hunk.stack_id,
                        commit_id: hunk.commit_id,
                        start: hunk.start,
                        lines: incoming_hunk.new_start - hunk.start,
                        line_shift: hunk.line_shift,
                    },
                    HunkRange {
                        change_type: incoming_hunk.change_type,
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
                        lines: hunk.start + hunk.lines
                            - self.get_shifted_old_start(incoming_hunk.old_start)
                            - incoming_hunk.old_lines,
                        line_shift: hunk.line_shift,
                    },
                ],
            );
            self.track_commit_dependency(commit_id, vec![hunk.commit_id])?;
            *index_next_hunk_to_visit = Some(index_of_next);
            self.update_start_lines(index_of_next, net_lines)?;
            return Ok(());
        }

        // 3. The incoming hunk partially overwrites the intersecting hunk range.
        let index_of_next = if self.get_shifted_old_start(incoming_hunk.old_start) <= hunk.start {
            // The incoming hunk overlaps the beginning of the intersecting hunk range.
            self.replace_hunk_ranges_at(
                index,
                vec![
                    HunkRange {
                        change_type: incoming_hunk.change_type,
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
                        lines: hunk.start + hunk.lines
                            - self.get_shifted_old_start(incoming_hunk.old_start)
                            - incoming_hunk.old_lines,
                        line_shift: net_lines,
                    },
                ],
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
                        lines: incoming_hunk.new_start - hunk.start,
                        line_shift: hunk.line_shift,
                    },
                    HunkRange {
                        change_type: incoming_hunk.change_type,
                        stack_id,
                        commit_id,
                        start: incoming_hunk.new_start,
                        lines: incoming_hunk.new_lines,
                        line_shift: net_lines,
                    },
                ],
            )
        };

        self.track_commit_dependency(commit_id, vec![hunk.commit_id])?;
        *index_next_hunk_to_visit = Some(index_of_next);
        self.update_start_lines(index_of_next, net_lines)?;

        Ok(())
    }

    fn track_commit_dependency(
        &mut self,
        commit_id: git2::Oid,
        parent_ids: Vec<git2::Oid>,
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
            let new_start = hunk.start as i32 + line_shift;
            if new_start < 0 {
                bail!("Hunk start is less than line shift");
            }
            hunk.start = new_start as u32;
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
        let shifted_old_start = old_start as i32 + self.line_shift;
        if shifted_old_start < 0 {
            0
        } else {
            shifted_old_start as u32
        }
    }

    /// Inserts the new hunks at the given index.
    ///
    /// Returns the index of the next hunk after the last added hunk.
    fn insert_hunk_ranges_at(&mut self, index: usize, hunks: Vec<HunkRange>) -> usize {
        insert_hunk_ranges(&mut self.hunk_ranges, index, index, hunks)
    }

    /// Replaces the hunk at the given index with the new hunks.
    ///
    /// Returns the index of the next hunk after the last added hunk.
    fn replace_hunk_ranges_at(&mut self, index: usize, hunks: Vec<HunkRange>) -> usize {
        insert_hunk_ranges(&mut self.hunk_ranges, index, index + 1, hunks)
    }

    /// Replaces the hunks between the given start and end indices with the new hunks.
    ///
    /// Returns the index of the next hunk after the last added hunk.
    fn replace_hunk_ranges_between(
        &mut self,
        start: usize,
        end: usize,
        hunks: Vec<HunkRange>,
    ) -> usize {
        insert_hunk_ranges(&mut self.hunk_ranges, start, end, hunks)
    }

    pub fn intersection(&self, start: u32, lines: u32) -> Vec<&HunkRange> {
        self.hunk_ranges
            .iter()
            .filter(|hunk| hunk.intersects(start, lines))
            .collect()
    }
}

/// Update the hunk ranges by inserting the new hunks at the given start and end indices.
///
/// Existing hunk ranges between the start and end indices are replaced by the new hunks.
/// Added hunk ranges that have 0 lines are ignored.
/// Returns the index of the next hunk after the last added hunk.
fn insert_hunk_ranges(
    hunk_ranges: &mut Vec<HunkRange>,
    start: usize,
    end: usize,
    hunks: Vec<HunkRange>,
) -> usize {
    let mut new_hunks = vec![];
    new_hunks.extend_from_slice(&hunk_ranges[..start]);

    let mut index_of_next = start;
    for hunk in hunks {
        if hunk.lines == 0 {
            // this will happen when a new diff completely
            // overwrites an existing hunk
            continue;
        }
        new_hunks.push(hunk);
        index_of_next += 1;
    }

    if end < hunk_ranges.len() {
        new_hunks.extend_from_slice(&hunk_ranges[end..]);
    }

    *hunk_ranges = new_hunks;

    index_of_next
}

#[cfg(test)]

mod tests {
    use super::*;

    #[test]
    fn test_replace_hunk_ranges_at_the_end() -> anyhow::Result<()> {
        let commit_a_id = git2::Oid::from_str("a")?;
        let mut hunk_ranges = vec![
            HunkRange {
                change_type: gitbutler_diff::ChangeType::Added,
                stack_id: StackId::generate(),
                commit_id: commit_a_id,
                start: 1,
                lines: 1,
                line_shift: 1,
            },
            HunkRange {
                change_type: gitbutler_diff::ChangeType::Modified,
                stack_id: StackId::generate(),
                commit_id: git2::Oid::from_str("b")?,
                start: 3,
                lines: 1,
                line_shift: 1,
            },
            HunkRange {
                change_type: gitbutler_diff::ChangeType::Modified,
                stack_id: StackId::generate(),
                commit_id: git2::Oid::from_str("b")?,
                start: 5,
                lines: 1,
                line_shift: 1,
            },
        ];

        let commit_c_id = git2::Oid::from_str("c")?;

        let hunks = vec![
            HunkRange {
                change_type: gitbutler_diff::ChangeType::Modified,
                stack_id: StackId::generate(),
                commit_id: commit_c_id,
                start: 2,
                lines: 1,
                line_shift: 1,
            },
            HunkRange {
                change_type: gitbutler_diff::ChangeType::Modified,
                stack_id: StackId::generate(),
                commit_id: commit_c_id,
                start: 4,
                lines: 1,
                line_shift: 1,
            },
        ];

        let index_of_next = insert_hunk_ranges(&mut hunk_ranges, 1, 5, hunks);

        assert_eq!(hunk_ranges.len(), 3);
        assert_eq!(index_of_next, 3);
        assert_eq!(hunk_ranges[0].commit_id, commit_a_id);
        assert_eq!(hunk_ranges[1].commit_id, commit_c_id);
        assert_eq!(hunk_ranges[2].commit_id, commit_c_id);

        Ok(())
    }

    #[test]
    fn test_replace_hunk_ranges_at_the_beginning() -> anyhow::Result<()> {
        let commit_a_id = git2::Oid::from_str("a")?;
        let commit_b_id = git2::Oid::from_str("b")?;
        let mut hunk_ranges = vec![
            HunkRange {
                change_type: gitbutler_diff::ChangeType::Added,
                stack_id: StackId::generate(),
                commit_id: commit_a_id,
                start: 1,
                lines: 1,
                line_shift: 1,
            },
            HunkRange {
                change_type: gitbutler_diff::ChangeType::Modified,
                stack_id: StackId::generate(),
                commit_id: commit_b_id,
                start: 3,
                lines: 1,
                line_shift: 1,
            },
            HunkRange {
                change_type: gitbutler_diff::ChangeType::Modified,
                stack_id: StackId::generate(),
                commit_id: commit_b_id,
                start: 5,
                lines: 1,
                line_shift: 1,
            },
        ];

        let commit_c_id = git2::Oid::from_str("c")?;

        let hunks = vec![
            HunkRange {
                change_type: gitbutler_diff::ChangeType::Modified,
                stack_id: StackId::generate(),
                commit_id: commit_c_id,
                start: 0,
                lines: 1,
                line_shift: 1,
            },
            HunkRange {
                change_type: gitbutler_diff::ChangeType::Modified,
                stack_id: StackId::generate(),
                commit_id: commit_c_id,
                start: 2,
                lines: 1,
                line_shift: 1,
            },
        ];

        let index_of_next = insert_hunk_ranges(&mut hunk_ranges, 0, 2, hunks);

        assert_eq!(hunk_ranges.len(), 3);
        assert_eq!(index_of_next, 2);
        assert_eq!(hunk_ranges[0].commit_id, commit_c_id);
        assert_eq!(hunk_ranges[1].commit_id, commit_c_id);
        assert_eq!(hunk_ranges[2].commit_id, commit_b_id);

        Ok(())
    }

    #[test]
    fn test_insert_single_hunk_range_at_the_beginning() -> anyhow::Result<()> {
        let commit_a_id = git2::Oid::from_str("a")?;
        let commit_b_id = git2::Oid::from_str("b")?;
        let commit_c_id = git2::Oid::from_str("c")?;

        let mut hunk_ranges = vec![
            HunkRange {
                change_type: gitbutler_diff::ChangeType::Added,
                stack_id: StackId::generate(),
                commit_id: commit_a_id,
                start: 1,
                lines: 1,
                line_shift: 1,
            },
            HunkRange {
                change_type: gitbutler_diff::ChangeType::Modified,
                stack_id: StackId::generate(),
                commit_id: commit_b_id,
                start: 3,
                lines: 1,
                line_shift: 1,
            },
            HunkRange {
                change_type: gitbutler_diff::ChangeType::Modified,
                stack_id: StackId::generate(),
                commit_id: commit_b_id,
                start: 5,
                lines: 1,
                line_shift: 1,
            },
        ];

        let hunks = vec![HunkRange {
            change_type: gitbutler_diff::ChangeType::Modified,
            stack_id: StackId::generate(),
            commit_id: commit_c_id,
            start: 0,
            lines: 1,
            line_shift: 1,
        }];

        let index_of_next = insert_hunk_ranges(&mut hunk_ranges, 0, 0, hunks);
        assert_eq!(hunk_ranges.len(), 4);
        assert_eq!(index_of_next, 1);
        assert_eq!(hunk_ranges[0].commit_id, commit_c_id);
        assert_eq!(hunk_ranges[1].commit_id, commit_a_id);
        assert_eq!(hunk_ranges[2].commit_id, commit_b_id);
        assert_eq!(hunk_ranges[3].commit_id, commit_b_id);

        Ok(())
    }

    #[test]
    fn test_insert_at_the_end() -> anyhow::Result<()> {
        let commit_a_id = git2::Oid::from_str("a")?;
        let commit_b_id = git2::Oid::from_str("b")?;
        let commit_c_id = git2::Oid::from_str("c")?;

        let mut hunk_ranges = vec![
            HunkRange {
                change_type: gitbutler_diff::ChangeType::Added,
                stack_id: StackId::generate(),
                commit_id: commit_a_id,
                start: 1,
                lines: 1,
                line_shift: 1,
            },
            HunkRange {
                change_type: gitbutler_diff::ChangeType::Modified,
                stack_id: StackId::generate(),
                commit_id: commit_b_id,
                start: 3,
                lines: 1,
                line_shift: 1,
            },
            HunkRange {
                change_type: gitbutler_diff::ChangeType::Modified,
                stack_id: StackId::generate(),
                commit_id: commit_b_id,
                start: 5,
                lines: 1,
                line_shift: 1,
            },
        ];

        let hunks = vec![HunkRange {
            change_type: gitbutler_diff::ChangeType::Modified,
            stack_id: StackId::generate(),
            commit_id: commit_c_id,
            start: 0,
            lines: 1,
            line_shift: 1,
        }];

        let index_of_next = insert_hunk_ranges(&mut hunk_ranges, 3, 3, hunks);
        assert_eq!(hunk_ranges.len(), 4);
        assert_eq!(index_of_next, 4);
        assert_eq!(hunk_ranges[0].commit_id, commit_a_id);
        assert_eq!(hunk_ranges[1].commit_id, commit_b_id);
        assert_eq!(hunk_ranges[2].commit_id, commit_b_id);
        assert_eq!(hunk_ranges[3].commit_id, commit_c_id);

        Ok(())
    }

    #[test]
    fn test_replace_hunk_ranges_between_add() -> anyhow::Result<()> {
        let commit_a_id = git2::Oid::from_str("a")?;
        let commit_b_id = git2::Oid::from_str("b")?;
        let commit_c_id = git2::Oid::from_str("c")?;

        let mut hunk_ranges = vec![
            HunkRange {
                change_type: gitbutler_diff::ChangeType::Added,
                stack_id: StackId::generate(),
                commit_id: commit_a_id,
                start: 1,
                lines: 1,
                line_shift: 1,
            },
            HunkRange {
                change_type: gitbutler_diff::ChangeType::Modified,
                stack_id: StackId::generate(),
                commit_id: commit_b_id,
                start: 3,
                lines: 1,
                line_shift: 1,
            },
            HunkRange {
                change_type: gitbutler_diff::ChangeType::Modified,
                stack_id: StackId::generate(),
                commit_id: commit_b_id,
                start: 5,
                lines: 1,
                line_shift: 1,
            },
        ];

        let hunks = vec![
            HunkRange {
                change_type: gitbutler_diff::ChangeType::Modified,
                stack_id: StackId::generate(),
                commit_id: commit_c_id,
                start: 0,
                lines: 1,
                line_shift: 1,
            },
            HunkRange {
                change_type: gitbutler_diff::ChangeType::Modified,
                stack_id: StackId::generate(),
                commit_id: commit_c_id,
                start: 2,
                lines: 1,
                line_shift: 1,
            },
        ];

        let index_of_next = insert_hunk_ranges(&mut hunk_ranges, 1, 2, hunks);
        assert_eq!(hunk_ranges.len(), 4);
        assert_eq!(index_of_next, 3);
        assert_eq!(hunk_ranges[0].commit_id, commit_a_id);
        assert_eq!(hunk_ranges[1].commit_id, commit_c_id);
        assert_eq!(hunk_ranges[2].commit_id, commit_c_id);
        assert_eq!(hunk_ranges[3].commit_id, commit_b_id);

        Ok(())
    }

    #[test]
    fn test_filter_out_single_range_replacement() -> anyhow::Result<()> {
        let commit_a_id = git2::Oid::from_str("a")?;
        let commit_b_id = git2::Oid::from_str("b")?;
        let commit_c_id = git2::Oid::from_str("c")?;

        let mut hunk_ranges = vec![
            HunkRange {
                change_type: gitbutler_diff::ChangeType::Added,
                stack_id: StackId::generate(),
                commit_id: commit_a_id,
                start: 1,
                lines: 1,
                line_shift: 1,
            },
            HunkRange {
                change_type: gitbutler_diff::ChangeType::Modified,
                stack_id: StackId::generate(),
                commit_id: commit_b_id,
                start: 3,
                lines: 1,
                line_shift: 1,
            },
            HunkRange {
                change_type: gitbutler_diff::ChangeType::Modified,
                stack_id: StackId::generate(),
                commit_id: commit_b_id,
                start: 5,
                lines: 1,
                line_shift: 1,
            },
        ];

        let hunks = vec![HunkRange {
            change_type: gitbutler_diff::ChangeType::Modified,
            stack_id: StackId::generate(),
            commit_id: commit_c_id,
            start: 4,
            lines: 0, // ranges with 0 lines are filtered out
            line_shift: 1,
        }];

        let index_of_next = insert_hunk_ranges(&mut hunk_ranges, 1, 2, hunks);
        assert_eq!(hunk_ranges.len(), 2);
        assert_eq!(index_of_next, 1);
        assert_eq!(hunk_ranges[0].commit_id, commit_a_id);
        assert_eq!(hunk_ranges[1].commit_id, commit_b_id);
        assert_eq!(hunk_ranges[1].start, 5);

        Ok(())
    }

    #[test]
    fn test_filter_out_multiple_range_replacement() -> anyhow::Result<()> {
        let commit_a_id = git2::Oid::from_str("a")?;
        let commit_b_id = git2::Oid::from_str("b")?;
        let commit_c_id = git2::Oid::from_str("c")?;

        let mut hunk_ranges = vec![
            HunkRange {
                change_type: gitbutler_diff::ChangeType::Added,
                stack_id: StackId::generate(),
                commit_id: commit_a_id,
                start: 1,
                lines: 1,
                line_shift: 1,
            },
            HunkRange {
                change_type: gitbutler_diff::ChangeType::Modified,
                stack_id: StackId::generate(),
                commit_id: commit_b_id,
                start: 3,
                lines: 1,
                line_shift: 1,
            },
            HunkRange {
                change_type: gitbutler_diff::ChangeType::Modified,
                stack_id: StackId::generate(),
                commit_id: commit_b_id,
                start: 5,
                lines: 1,
                line_shift: 1,
            },
        ];

        let hunks = vec![
            HunkRange {
                change_type: gitbutler_diff::ChangeType::Modified,
                stack_id: StackId::generate(),
                commit_id: commit_c_id,
                start: 4,
                lines: 0, // ranges with 0 lines are filtered out
                line_shift: 1,
            },
            HunkRange {
                change_type: gitbutler_diff::ChangeType::Modified,
                stack_id: StackId::generate(),
                commit_id: commit_c_id,
                start: 5,
                lines: 3,
                line_shift: 1,
            },
            HunkRange {
                change_type: gitbutler_diff::ChangeType::Modified,
                stack_id: StackId::generate(),
                commit_id: commit_c_id,
                start: 6,
                lines: 0, // ranges with 0 lines are filtered out
                line_shift: 1,
            },
        ];

        let index_of_next = insert_hunk_ranges(&mut hunk_ranges, 1, 2, hunks);
        assert_eq!(hunk_ranges.len(), 3);
        assert_eq!(index_of_next, 2);
        assert_eq!(hunk_ranges[0].commit_id, commit_a_id);
        assert_eq!(hunk_ranges[1].commit_id, commit_c_id);
        assert_eq!(hunk_ranges[1].start, 5);
        assert_eq!(hunk_ranges[2].commit_id, commit_b_id);

        Ok(())
    }

    #[test]
    fn test_filter_out_single_range_insertion() -> anyhow::Result<()> {
        let commit_a_id = git2::Oid::from_str("a")?;
        let commit_b_id = git2::Oid::from_str("b")?;
        let commit_c_id = git2::Oid::from_str("c")?;

        let mut hunk_ranges = vec![
            HunkRange {
                change_type: gitbutler_diff::ChangeType::Added,
                stack_id: StackId::generate(),
                commit_id: commit_a_id,
                start: 1,
                lines: 1,
                line_shift: 1,
            },
            HunkRange {
                change_type: gitbutler_diff::ChangeType::Modified,
                stack_id: StackId::generate(),
                commit_id: commit_b_id,
                start: 3,
                lines: 1,
                line_shift: 1,
            },
            HunkRange {
                change_type: gitbutler_diff::ChangeType::Modified,
                stack_id: StackId::generate(),
                commit_id: commit_b_id,
                start: 5,
                lines: 1,
                line_shift: 1,
            },
        ];

        let hunks = vec![HunkRange {
            change_type: gitbutler_diff::ChangeType::Modified,
            stack_id: StackId::generate(),
            commit_id: commit_c_id,
            start: 4,
            lines: 0, // ranges with 0 lines are filtered out
            line_shift: 1,
        }];

        let index_of_next = insert_hunk_ranges(&mut hunk_ranges, 1, 1, hunks);
        assert_eq!(hunk_ranges.len(), 3);
        assert_eq!(index_of_next, 1);
        assert_eq!(hunk_ranges[0].commit_id, commit_a_id);
        assert_eq!(hunk_ranges[1].commit_id, commit_b_id);
        assert_eq!(hunk_ranges[2].commit_id, commit_b_id);

        Ok(())
    }

    #[test]
    fn test_filter_out_multiple_range_insertion() -> anyhow::Result<()> {
        let commit_a_id = git2::Oid::from_str("a")?;
        let commit_b_id = git2::Oid::from_str("b")?;
        let commit_c_id = git2::Oid::from_str("c")?;

        let mut hunk_ranges = vec![
            HunkRange {
                change_type: gitbutler_diff::ChangeType::Added,
                stack_id: StackId::generate(),
                commit_id: commit_a_id,
                start: 1,
                lines: 1,
                line_shift: 1,
            },
            HunkRange {
                change_type: gitbutler_diff::ChangeType::Modified,
                stack_id: StackId::generate(),
                commit_id: commit_b_id,
                start: 3,
                lines: 1,
                line_shift: 1,
            },
            HunkRange {
                change_type: gitbutler_diff::ChangeType::Modified,
                stack_id: StackId::generate(),
                commit_id: commit_b_id,
                start: 5,
                lines: 1,
                line_shift: 1,
            },
        ];

        let hunks = vec![
            HunkRange {
                change_type: gitbutler_diff::ChangeType::Modified,
                stack_id: StackId::generate(),
                commit_id: commit_c_id,
                start: 4,
                lines: 0, // ranges with 0 lines are filtered out
                line_shift: 1,
            },
            HunkRange {
                change_type: gitbutler_diff::ChangeType::Modified,
                stack_id: StackId::generate(),
                commit_id: commit_c_id,
                start: 5,
                lines: 3,
                line_shift: 1,
            },
            HunkRange {
                change_type: gitbutler_diff::ChangeType::Modified,
                stack_id: StackId::generate(),
                commit_id: commit_c_id,
                start: 6,
                lines: 0, // ranges with 0 lines are filtered out
                line_shift: 1,
            },
            HunkRange {
                change_type: gitbutler_diff::ChangeType::Modified,
                stack_id: StackId::generate(),
                commit_id: commit_c_id,
                start: 8,
                lines: 3,
                line_shift: 1,
            },
        ];

        let index_of_next = insert_hunk_ranges(&mut hunk_ranges, 1, 1, hunks);
        assert_eq!(hunk_ranges.len(), 5);
        assert_eq!(index_of_next, 3);
        assert_eq!(hunk_ranges[0].commit_id, commit_a_id);
        assert_eq!(hunk_ranges[1].commit_id, commit_c_id);
        assert_eq!(hunk_ranges[2].commit_id, commit_c_id);
        assert_eq!(hunk_ranges[3].commit_id, commit_b_id);
        assert_eq!(hunk_ranges[4].commit_id, commit_b_id);

        Ok(())
    }
}
