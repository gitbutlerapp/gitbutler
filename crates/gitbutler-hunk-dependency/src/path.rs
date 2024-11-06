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
    pub hunks: Vec<HunkRange>,
    pub commit_dependencies: HashMap<git2::Oid, HashSet<git2::Oid>>,
    commit_ids: Vec<git2::Oid>,
    line_shift: i32,
}

impl PathRanges {
    pub fn add(
        &mut self,
        stack_id: StackId,
        commit_id: git2::Oid,
        diffs: Vec<InputDiff>,
    ) -> anyhow::Result<()> {
        if self.commit_ids.contains(&commit_id) {
            bail!("Commit ID already in stack: {}", commit_id)
        }

        let mut index_next_hunk_to_visit: Option<usize> = None;
        let diffs_count = diffs.len();
        self.line_shift = 0;

        // This is the main loop that processes all diff hunks in a commit,
        // turning them into hunk ranges and inserting them in order.
        for diff in &diffs {
            // handle existing hunk is a file deletion
            if self.hunks.len() == 1
                && self.hunks[0].change_type == gitbutler_diff::ChangeType::Deleted
            {
                self.handle_file_recreation(commit_id, stack_id, diffs, self.hunks[0])?;
                break;
            }

            // assume that a diff deleting a file is the only diff in the commit
            if diff.change_type == gitbutler_diff::ChangeType::Deleted {
                self.handle_file_deletion(diffs_count, diff, stack_id, commit_id)?;
                break;
            }

            // if no existing hunks, add all diffs
            if self.hunks.is_empty() {
                self.handle_add_all_diffs(stack_id, commit_id, diffs)?;
                break;
            }

            // find all existing hunks that intersect with the new diff

            // if we already added a diff, we need to check **only** the diffs after that.
            // -> diffs are expected to be added in top to bottom and that they don't overlap

            if let Some(i) = index_next_hunk_to_visit {
                // if the last diff was added at the end, there are no more hunks to compare against.
                // -> we can just append the new diff
                if i >= self.hunks.len() {
                    // append the new diff depends only of the commit that created the file (if any)
                    let file_creation_commit = self.find_file_creation_commit();

                    if let Some(file_creation_commit) = file_creation_commit {
                        self.track_commit_dependency(commit_id, vec![file_creation_commit])?;
                    }

                    if diff.new_lines > 0 {
                        self.hunks.push(HunkRange {
                            change_type: diff.change_type,
                            stack_id,
                            commit_id,
                            start: diff.new_start,
                            lines: diff.new_lines,
                            line_shift: diff.net_lines()?,
                        });
                    }

                    index_next_hunk_to_visit = Some(self.hunks.len());
                    continue;
                }
            }

            // start looking for intersecting hunks after the last added diff if there is one,
            // otherwise start from the beginning
            let mut i = index_next_hunk_to_visit.unwrap_or_default();

            // find all intersecting hunks
            let mut intersecting_hunks = vec![];
            while i < self.hunks.len() {
                let current_hunk = self.hunks[i];

                // current hunk is below the new diff
                // -> we can stop looking for intersecting hunks
                if current_hunk.follows(self.get_shifted_old_start(diff.old_start), diff.old_lines)
                {
                    break;
                }

                // current hunk is above the new diff
                // and doesn't intersect with it
                if current_hunk.precedes(self.get_shifted_old_start(diff.old_start)) {
                    i += 1;
                    continue;
                }

                if current_hunk
                    .intersects(self.get_shifted_old_start(diff.old_start), diff.old_lines)
                {
                    intersecting_hunks.push((i, current_hunk));
                }

                i += 1;
            }

            // if there are no intersecting hunks, we just add the new diff
            if intersecting_hunks.is_empty() {
                self.handle_no_intersecting_hunks(
                    commit_id,
                    i,
                    diff,
                    stack_id,
                    &mut index_next_hunk_to_visit,
                )?;
                continue;
            }

            // if there are intersecting hunks, we need to handle them
            if intersecting_hunks.len() == 1 {
                self.handle_single_intersecting_hunk(
                    intersecting_hunks[0],
                    diff,
                    stack_id,
                    commit_id,
                    &mut index_next_hunk_to_visit,
                )?;
                continue;
            }

            self.handle_multiple_intersecting_hunks(
                intersecting_hunks,
                diff,
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
        diffs: Vec<InputDiff>,
        existing_hunk: HunkRange,
    ) -> Result<(), anyhow::Error> {
        if diffs.len() > 1 {
            bail!("File recreation must be the only diff in a commit");
        }
        let diff = &diffs[0];
        if diff.change_type != gitbutler_diff::ChangeType::Added {
            bail!("File recreation must be an addition");
        }

        self.track_commit_dependency(commit_id, vec![existing_hunk.commit_id])?;
        self.hunks.clear();
        self.handle_add_all_diffs(stack_id, commit_id, diffs)?;
        Ok(())
    }

    fn handle_file_deletion(
        &mut self,
        diffs_count: usize,
        diff: &InputDiff,
        stack_id: gitbutler_id::id::Id<gitbutler_stack::Stack>,
        commit_id: git2::Oid,
    ) -> Result<(), anyhow::Error> {
        // New diff is a file deletion.
        // This overrides all existing hunks.
        if diffs_count > 1 {
            bail!("File deletion must be the only diff in a commit");
        }
        self.hunks = vec![HunkRange {
            change_type: diff.change_type,
            stack_id,
            commit_id,
            start: diff.new_start,
            lines: diff.new_lines,
            line_shift: 0,
        }];

        // The commit that deletes a file depends on the last commit that touched it.
        if let Some(previous_commit_added) = self.commit_ids.last().copied() {
            self.track_commit_dependency(commit_id, vec![previous_commit_added])?;
        }

        Ok(())
    }

    fn handle_no_intersecting_hunks(
        &mut self,
        commit_id: git2::Oid,
        index: usize,
        diff: &InputDiff,
        stack_id: gitbutler_id::id::Id<gitbutler_stack::Stack>,
        index_next_hunk_to_visit: &mut Option<usize>,
    ) -> Result<(), anyhow::Error> {
        // The new diff does not intersect with anything.
        // The only commit that this depends on is the commit that created the file.
        // That commit may or may not be available in the hunk list.
        let file_creation_commit = self.find_file_creation_commit();

        if let Some(file_creation_commit) = file_creation_commit {
            self.track_commit_dependency(commit_id, vec![file_creation_commit])?;
        }

        let index_of_next = self.insert_hunk_ranges_at(
            index,
            vec![HunkRange {
                change_type: diff.change_type,
                stack_id,
                commit_id,
                start: diff.new_start,
                lines: diff.new_lines,
                line_shift: diff.net_lines()?,
            }],
        );
        *index_next_hunk_to_visit = Some(index_of_next);
        self.update_start_lines(index_of_next, diff.net_lines()?)?;
        Ok(())
    }

    /// Look for the commit that created the file.
    fn find_file_creation_commit(&mut self) -> Option<git2::Oid> {
        let file_creation_commit = self
            .hunks
            .iter()
            .find(|h| h.change_type == gitbutler_diff::ChangeType::Added)
            .map(|h| h.commit_id);
        file_creation_commit
    }

    fn handle_add_all_diffs(
        &mut self,
        stack_id: gitbutler_id::id::Id<gitbutler_stack::Stack>,
        commit_id: git2::Oid,
        diffs: Vec<InputDiff>,
    ) -> anyhow::Result<()> {
        for diff in diffs {
            self.hunks.push(HunkRange {
                change_type: diff.change_type,
                stack_id,
                commit_id,
                start: diff.new_start,
                lines: diff.new_lines,
                line_shift: diff.net_lines()?,
            });
        }
        Ok(())
    }

    /// Added diff affects multiple hunks.
    fn handle_multiple_intersecting_hunks(
        &mut self,
        intersecting_hunks: Vec<(usize, HunkRange)>,
        diff: &InputDiff,
        stack_id: gitbutler_id::id::Id<gitbutler_stack::Stack>,
        commit_id: git2::Oid,
        index_next_hunk_to_visit: &mut Option<usize>,
    ) -> anyhow::Result<()> {
        // if there are multiple intersecting hunks, we can ignore all the intersecting hunks
        // in the middle as they are considered to be completely overwritten by the new diff.
        let net_lines = diff.net_lines()?;
        let affected_commits = intersecting_hunks
            .iter()
            .map(|(_, hunk)| hunk.commit_id)
            .collect::<HashSet<_>>();

        // there are two possibilities:
        let (first_intersecting_hunk_index, first_intersecting_hunk) =
            intersecting_hunks
                .first()
                .ok_or(anyhow::anyhow!("No first intersecting hunk"))?;
        let (last_intersecting_hunk_index, last_intersecting_hunk) = intersecting_hunks
            .last()
            .ok_or(anyhow::anyhow!("No last intersecting hunk"))?;

        // 1. the new diff completely overwrites the intersecting hunks
        if first_intersecting_hunk
            .covered_by(self.get_shifted_old_start(diff.old_start), diff.old_lines)
            && last_intersecting_hunk
                .covered_by(self.get_shifted_old_start(diff.old_start), diff.old_lines)
        {
            let index_of_next = self.replace_hunk_ranges_between(
                *first_intersecting_hunk_index,
                *last_intersecting_hunk_index + 1,
                vec![HunkRange {
                    change_type: diff.change_type,
                    stack_id,
                    commit_id,
                    start: diff.new_start,
                    lines: diff.new_lines,
                    line_shift: net_lines,
                }],
            );

            self.track_commit_dependency(commit_id, affected_commits.into_iter().collect())?;
            *index_next_hunk_to_visit = Some(index_of_next);
            self.update_start_lines(index_of_next, net_lines)?;
            return Ok(());
        }

        // 2. the new diff partially overwrites the intersecting hunks
        // 2.1. the new diff overlaps the beginning of the second intersecting hunk
        if first_intersecting_hunk
            .covered_by(self.get_shifted_old_start(diff.old_start), diff.old_lines)
        {
            let index_of_next = self.replace_hunk_ranges_between(
                *first_intersecting_hunk_index,
                *last_intersecting_hunk_index + 1,
                vec![
                    HunkRange {
                        change_type: diff.change_type,
                        stack_id,
                        commit_id,
                        start: diff.new_start,
                        lines: diff.new_lines,
                        line_shift: net_lines,
                    },
                    HunkRange {
                        change_type: last_intersecting_hunk.change_type,
                        stack_id: last_intersecting_hunk.stack_id,
                        commit_id: last_intersecting_hunk.commit_id,
                        start: diff.new_start + diff.new_lines,
                        lines: last_intersecting_hunk.start + last_intersecting_hunk.lines
                            - self.get_shifted_old_start(diff.old_start)
                            - diff.old_lines,
                        line_shift: last_intersecting_hunk.line_shift,
                    },
                ],
            );
            self.track_commit_dependency(commit_id, affected_commits.into_iter().collect())?;
            *index_next_hunk_to_visit = Some(index_of_next);
            self.update_start_lines(index_of_next, net_lines)?;
            return Ok(());
        }

        // 2.2. the new diff overlaps the end of the first intersecting hunk
        if last_intersecting_hunk
            .covered_by(self.get_shifted_old_start(diff.old_start), diff.old_lines)
        {
            let index_of_next = self.replace_hunk_ranges_between(
                *first_intersecting_hunk_index,
                *last_intersecting_hunk_index + 1,
                vec![
                    HunkRange {
                        change_type: first_intersecting_hunk.change_type,
                        stack_id: first_intersecting_hunk.stack_id,
                        commit_id: first_intersecting_hunk.commit_id,
                        start: first_intersecting_hunk.start,
                        lines: diff.new_start - first_intersecting_hunk.start,
                        line_shift: first_intersecting_hunk.line_shift,
                    },
                    HunkRange {
                        change_type: diff.change_type,
                        stack_id,
                        commit_id,
                        start: diff.new_start,
                        lines: diff.new_lines,
                        line_shift: net_lines,
                    },
                ],
            );
            self.track_commit_dependency(commit_id, affected_commits.into_iter().collect())?;
            *index_next_hunk_to_visit = Some(index_of_next);
            self.update_start_lines(index_of_next, net_lines)?;
            return Ok(());
        }

        // 2.3. the new diff is contained in the intersecting hunks
        let index_of_next = self.replace_hunk_ranges_between(
            *first_intersecting_hunk_index,
            *last_intersecting_hunk_index + 1,
            vec![
                HunkRange {
                    change_type: first_intersecting_hunk.change_type,
                    stack_id: first_intersecting_hunk.stack_id,
                    commit_id: first_intersecting_hunk.commit_id,
                    start: first_intersecting_hunk.start,
                    lines: diff.new_start - first_intersecting_hunk.start,
                    line_shift: first_intersecting_hunk.line_shift,
                },
                HunkRange {
                    change_type: diff.change_type,
                    stack_id,
                    commit_id,
                    start: diff.new_start,
                    lines: diff.new_lines,
                    line_shift: net_lines,
                },
                HunkRange {
                    change_type: last_intersecting_hunk.change_type,
                    stack_id: last_intersecting_hunk.stack_id,
                    commit_id: last_intersecting_hunk.commit_id,
                    start: diff.new_start + diff.new_lines,
                    lines: last_intersecting_hunk.start + last_intersecting_hunk.lines
                        - self.get_shifted_old_start(diff.old_start)
                        - diff.old_lines,
                    line_shift: last_intersecting_hunk.line_shift,
                },
            ],
        );
        self.track_commit_dependency(commit_id, affected_commits.into_iter().collect())?;
        *index_next_hunk_to_visit = Some(index_of_next);
        self.update_start_lines(index_of_next, net_lines)?;

        Ok(())
    }

    /// Added diff only affects a single hunk.
    fn handle_single_intersecting_hunk(
        &mut self,
        intersecting_hunk: (usize, HunkRange),
        diff: &InputDiff,
        stack_id: gitbutler_id::id::Id<gitbutler_stack::Stack>,
        commit_id: git2::Oid,
        index_next_hunk_to_visit: &mut Option<usize>,
    ) -> anyhow::Result<()> {
        // if there is only one intersecting hunk there are three possibilities:
        let (index, hunk) = intersecting_hunk;
        let net_lines = diff.net_lines()?;

        // 1. the new diff completely overwrites the intersecting hunk
        if hunk.covered_by(self.get_shifted_old_start(diff.old_start), diff.old_lines) {
            let index_of_next = self.replace_hunk_ranges_at(
                index,
                vec![HunkRange {
                    change_type: diff.change_type,
                    stack_id,
                    commit_id,
                    start: diff.new_start,
                    lines: diff.new_lines,
                    line_shift: net_lines,
                }],
            );

            self.track_commit_dependency(commit_id, vec![hunk.commit_id])?;
            *index_next_hunk_to_visit = Some(index_of_next);
            self.update_start_lines(index_of_next, net_lines)?;
            return Ok(());
        }

        // 2. the new diff is contained in the intersecting hunk
        if hunk.contains(self.get_shifted_old_start(diff.old_start), diff.old_lines) {
            let index_of_next = self.replace_hunk_ranges_at(
                index,
                vec![
                    HunkRange {
                        change_type: hunk.change_type,
                        stack_id: hunk.stack_id,
                        commit_id: hunk.commit_id,
                        start: hunk.start,
                        lines: diff.new_start - hunk.start,
                        line_shift: hunk.line_shift,
                    },
                    HunkRange {
                        change_type: diff.change_type,
                        stack_id,
                        commit_id,
                        start: diff.new_start,
                        lines: diff.new_lines,
                        line_shift: net_lines,
                    },
                    HunkRange {
                        change_type: hunk.change_type,
                        stack_id: hunk.stack_id,
                        commit_id: hunk.commit_id,
                        start: diff.new_start + diff.new_lines,
                        lines: hunk.start + hunk.lines
                            - self.get_shifted_old_start(diff.old_start)
                            - diff.old_lines,
                        line_shift: hunk.line_shift,
                    },
                ],
            );
            self.track_commit_dependency(commit_id, vec![hunk.commit_id])?;
            *index_next_hunk_to_visit = Some(index_of_next);
            self.update_start_lines(index_of_next, net_lines)?;
            return Ok(());
        }

        // 3. the new diff partially overwrites the intersecting hunk
        let index_of_next = if self.get_shifted_old_start(diff.old_start) <= hunk.start {
            // The new diff overlaps the beginning of the intersecting hunk.
            self.replace_hunk_ranges_at(
                index,
                vec![
                    HunkRange {
                        change_type: diff.change_type,
                        stack_id,
                        commit_id,
                        start: diff.new_start,
                        lines: diff.new_lines,
                        line_shift: net_lines,
                    },
                    HunkRange {
                        change_type: hunk.change_type,
                        stack_id: hunk.stack_id,
                        commit_id: hunk.commit_id,
                        start: diff.new_start + diff.new_lines,
                        lines: hunk.start + hunk.lines
                            - self.get_shifted_old_start(diff.old_start)
                            - diff.old_lines,
                        line_shift: net_lines,
                    },
                ],
            )
        } else {
            // The new diff overlaps the end of the intersecting hunk.
            self.replace_hunk_ranges_at(
                index,
                vec![
                    HunkRange {
                        change_type: hunk.change_type,
                        stack_id: hunk.stack_id,
                        commit_id: hunk.commit_id,
                        start: hunk.start,
                        lines: diff.new_start - hunk.start,
                        line_shift: hunk.line_shift,
                    },
                    HunkRange {
                        change_type: diff.change_type,
                        stack_id,
                        commit_id,
                        start: diff.new_start,
                        lines: diff.new_lines,
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

        if index_of_first_hunk >= self.hunks.len() {
            return Ok(());
        }
        for hunk in &mut self.hunks[index_of_first_hunk..] {
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
        let mut new_hunks = vec![];
        new_hunks.extend_from_slice(&self.hunks[..index]);

        let mut index_of_next = index;
        for hunk in hunks {
            if hunk.lines == 0 {
                // this will happen when a new diff completely
                // overwrites an existing hunk
                continue;
            }
            new_hunks.push(hunk);
            index_of_next += 1;
        }

        new_hunks.extend_from_slice(&self.hunks[index..]);

        self.hunks = new_hunks;

        index_of_next
    }

    /// Replaces the hunk at the given index with the new hunks.
    ///
    /// Returns the index of the next hunk after the last added hunk.
    fn replace_hunk_ranges_at(&mut self, index: usize, hunks: Vec<HunkRange>) -> usize {
        let mut new_hunks = vec![];
        new_hunks.extend_from_slice(&self.hunks[..index]);

        let mut index_of_next = index;
        for hunk in hunks {
            if hunk.lines == 0 {
                // this will happen when a new diff completely
                // overwrites an existing hunk
                continue;
            }
            new_hunks.push(hunk);
            index_of_next += 1;
        }

        if index + 1 < self.hunks.len() {
            new_hunks.extend_from_slice(&self.hunks[index + 1..]);
        }

        self.hunks = new_hunks;

        index_of_next
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
        let mut new_hunks = vec![];
        new_hunks.extend_from_slice(&self.hunks[..start]);

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

        if end < self.hunks.len() {
            new_hunks.extend_from_slice(&self.hunks[end..]);
        }
        self.hunks = new_hunks;

        index_of_next
    }

    pub fn intersection(&self, start: u32, lines: u32) -> Vec<&HunkRange> {
        self.hunks
            .iter()
            .filter(|hunk| hunk.intersects(start, lines))
            .collect()
    }
}
