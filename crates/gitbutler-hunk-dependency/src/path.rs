use std::collections::HashSet;

use gitbutler_stack::StackId;

use crate::{diff::InputDiff, hunk::HunkRange};

#[derive(Debug, Default, PartialEq, Clone)]
pub struct PathHunkRanges {
    pub hunks: Vec<HunkRange>,
    commit_ids: HashSet<git2::Oid>,
}

impl PathHunkRanges {
    pub fn find(&mut self, start: i32, lines: i32) -> Vec<&mut HunkRange> {
        self.hunks
            .iter_mut()
            .filter(|hunk| hunk.intersects(start, lines))
            .collect()
    }

    pub fn add(&mut self, stack_id: StackId, commit_id: git2::Oid, diffs: Vec<InputDiff>) {
        if !self.commit_ids.insert(commit_id) {
            panic!("Commit ID already in stack: {}", commit_id)
        }

        let mut line_shift = 0;
        let mut new_hunks: Vec<HunkRange> = vec![];
        let mut last_hunk: Option<HunkRange> = None;

        let [mut i, mut j] = [0, 0];

        while i < diffs.len() || j < self.hunks.len() {
            // If the old start is smaller than existing new_start, or if only have
            // new diffs left to process.
            let mut hunks = if (i < diffs.len()
                && j < self.hunks.len()
                && diffs[i].old_start < self.hunks[j].start)
                || (i < diffs.len() && j >= self.hunks.len())
            {
                i += 1;
                // TODO: Should we add line shift before or after?
                line_shift += diffs[i - 1].net_lines();
                add_new(&diffs[i - 1], last_hunk, stack_id, commit_id)
            } else {
                j += 1;
                add_existing(&self.hunks[j - 1], last_hunk, line_shift)
            };
            // Last node is needed when adding new one, so we delay inserting it.
            last_hunk = hunks.pop();
            new_hunks.extend(hunks);
        }

        if let Some(last_hunk) = last_hunk {
            new_hunks.push(last_hunk);
        };

        self.hunks = new_hunks;
    }
}

fn add_new(
    new_diff: &InputDiff,
    last_hunk: Option<HunkRange>,
    stack_id: StackId,
    commit_id: git2::Oid,
) -> Vec<HunkRange> {
    // If we have nothing to compare against we just return the new diff.
    if last_hunk.is_none() {
        return vec![HunkRange {
            stack_id,
            commit_id,
            start: new_diff.new_start,
            lines: new_diff.new_lines,
            line_shift: new_diff.net_lines(),
        }];
    }

    // TODO: Is the above early return idiomatic? Using unwrap here to avoid nesting.
    let last_hunk = last_hunk.unwrap();

    if last_hunk.start + last_hunk.lines < new_diff.old_start {
        // Diffs do not overlap so we return them in order.
        vec![
            last_hunk.clone(),
            HunkRange {
                commit_id,
                stack_id,
                start: new_diff.new_start,
                lines: new_diff.new_lines,
                line_shift: new_diff.net_lines(),
            },
        ]
    } else if last_hunk.contains(new_diff.old_start, new_diff.old_lines) {
        // Since the diff being added is from the current commit it
        // overwrites the preceding one, but we need to split it in
        // two and retain the tail.
        vec![
            HunkRange {
                commit_id: last_hunk.commit_id,
                stack_id: last_hunk.stack_id,
                start: last_hunk.start,
                lines: new_diff.new_start - last_hunk.start,
                line_shift: 0,
            },
            HunkRange {
                commit_id,
                stack_id,
                start: new_diff.new_start,
                lines: new_diff.new_lines,
                line_shift: new_diff.net_lines(),
            },
            HunkRange {
                commit_id: last_hunk.commit_id,
                stack_id: last_hunk.stack_id,
                start: new_diff.new_start + new_diff.new_lines,
                lines: last_hunk.start + last_hunk.lines
                    - (new_diff.new_start + new_diff.new_lines),
                line_shift: last_hunk.line_shift,
            },
        ]
    } else {
        vec![
            HunkRange {
                commit_id: last_hunk.commit_id,
                stack_id: last_hunk.stack_id,
                start: last_hunk.start,
                lines: last_hunk.lines,
                line_shift: last_hunk.line_shift,
            },
            HunkRange {
                commit_id,
                stack_id,
                start: new_diff.new_start,
                lines: new_diff.new_lines,
                line_shift: new_diff.net_lines(),
            },
        ]
    }
}

fn add_existing(hunk: &HunkRange, last_hunk: Option<HunkRange>, shift: i32) -> Vec<HunkRange> {
    if last_hunk.is_none() {
        return vec![hunk.clone()];
    };

    let last_hunk = last_hunk.unwrap();
    if hunk.start > last_hunk.start + last_hunk.lines {
        vec![
            last_hunk.clone(),
            HunkRange {
                commit_id: hunk.commit_id,
                stack_id: hunk.stack_id,
                start: hunk.start + shift,
                lines: hunk.lines,
                line_shift: hunk.line_shift,
            },
        ]
    } else if last_hunk.contains(hunk.start, hunk.lines) {
        vec![last_hunk.clone()]
    } else {
        vec![
            last_hunk.clone(),
            HunkRange {
                commit_id: hunk.commit_id,
                stack_id: hunk.stack_id,
                start: hunk.start + shift,
                lines: hunk.lines - (last_hunk.start + last_hunk.lines - hunk.start),
                line_shift: hunk.line_shift,
            },
        ]
    }
}
