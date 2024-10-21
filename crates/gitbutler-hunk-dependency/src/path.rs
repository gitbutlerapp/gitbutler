use std::collections::HashSet;

use anyhow::bail;
use gitbutler_stack::StackId;

use crate::{HunkRange, InputDiff};

/// Adds sequential diffs from sequential commits for a specific path, and
/// shifts line numbers with additions and deletions. It is expected that
/// diffs are added one commit at a time, each time merging the already added
/// diffs with the new ones being added.
///
/// When combining old and new diffs we process them in turn of their start
/// line, lowest first. With each addition it is possible that we conflict
/// with previous ranges (we only know start line is higher, line count can
/// be very different), but it is important to note that old ranges will
/// not conflict with old ranges, and new ranges cannot conflict with new
/// ranges.
///
/// Therefore, a) if we are processing a new diff we know it overwrites
/// anything it conflicts with, b) when processing an old diff we e.g.
/// omit it if has been overwritten.
#[derive(Debug, Default, PartialEq, Clone)]
pub struct PathRanges {
    pub hunks: Vec<HunkRange>,
    commit_ids: HashSet<git2::Oid>,
}

impl PathRanges {
    pub fn add(
        &mut self,
        stack_id: StackId,
        commit_id: git2::Oid,
        diffs: Vec<InputDiff>,
    ) -> anyhow::Result<()> {
        if !self.commit_ids.insert(commit_id) {
            bail!("Commit ID already in stack: {}", commit_id)
        }

        // Cumulative count of net line change, used to update start lines.
        let mut net_lines = 0;
        let mut new_hunks: Vec<HunkRange> = vec![];
        let mut last_hunk: Option<HunkRange> = None;

        // Two pointers for iterating over two arrays.
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
                net_lines += diffs[i - 1].net_lines()?;
                add_new(&diffs[i - 1], last_hunk, stack_id, commit_id)?
            } else {
                j += 1;
                add_existing(&self.hunks[j - 1], last_hunk, net_lines)
            };
            // Last node is needed when adding new one, so we delay inserting it.
            last_hunk = hunks.pop();
            new_hunks.extend(hunks);
        }

        if let Some(last_hunk) = last_hunk {
            new_hunks.push(last_hunk);
        };

        self.hunks = new_hunks;
        Ok(())
    }

    pub fn intersection(&mut self, start: u32, lines: u32) -> Vec<&mut HunkRange> {
        self.hunks
            .iter_mut()
            .filter(|hunk| hunk.intersects(start, lines))
            .collect()
    }
}

/// Determines how to add new diff given the previous one.
fn add_new(
    new_diff: &InputDiff,
    last_hunk: Option<HunkRange>,
    stack_id: StackId,
    commit_id: git2::Oid,
) -> anyhow::Result<Vec<HunkRange>> {
    // If we have nothing to compare against we just return the new diff.
    if last_hunk.is_none() {
        return Ok(vec![HunkRange {
            stack_id,
            commit_id,
            start: new_diff.new_start,
            lines: new_diff.new_lines,
            line_shift: new_diff.net_lines()?,
        }]);
    }
    let last_hunk = last_hunk.unwrap();

    if last_hunk.start + last_hunk.lines < new_diff.old_start {
        // Diffs do not overlap so we return them in order.
        Ok(vec![
            last_hunk.clone(),
            HunkRange {
                commit_id,
                stack_id,
                start: new_diff.new_start,
                lines: new_diff.new_lines,
                line_shift: new_diff.net_lines()?,
            },
        ])
    } else if last_hunk.contains(new_diff.old_start, new_diff.old_lines) {
        // Since the diff being added is from the current commit it overwrites the
        // preceding one, but we need to split it in two and retain the tail.
        Ok(vec![
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
                line_shift: new_diff.net_lines()?,
            },
            HunkRange {
                commit_id: last_hunk.commit_id,
                stack_id: last_hunk.stack_id,
                start: new_diff.new_start + new_diff.new_lines,
                lines: last_hunk.lines
                    - new_diff.old_lines
                    - (new_diff.old_start - last_hunk.start),
                line_shift: last_hunk.line_shift,
            },
        ])
    } else {
        // Overwrite the tail of the previous diff.
        Ok(vec![
            HunkRange {
                commit_id: last_hunk.commit_id,
                stack_id: last_hunk.stack_id,
                start: last_hunk.start,
                lines: new_diff.new_start - last_hunk.start,
                line_shift: last_hunk.line_shift,
            },
            HunkRange {
                commit_id,
                stack_id,
                start: new_diff.new_start,
                lines: new_diff.new_lines,
                line_shift: new_diff.net_lines()?,
            },
        ])
    }
}

/// Determines how existing diff given the previous one.
fn add_existing(hunk: &HunkRange, last_hunk: Option<HunkRange>, shift: i32) -> Vec<HunkRange> {
    if last_hunk.is_none() {
        return vec![hunk.clone()];
    };
    let last_hunk = last_hunk.unwrap();

    if hunk.start.saturating_add_signed(shift) > last_hunk.start + last_hunk.lines {
        vec![
            last_hunk.clone(),
            HunkRange {
                commit_id: hunk.commit_id,
                stack_id: hunk.stack_id,
                start: hunk.start.saturating_add_signed(shift),
                lines: hunk.lines,
                line_shift: hunk.line_shift,
            },
        ]
    } else if last_hunk.contains(hunk.start.saturating_add_signed(shift), hunk.lines) {
        vec![last_hunk.clone()]
    } else {
        vec![
            last_hunk.clone(),
            HunkRange {
                commit_id: hunk.commit_id,
                stack_id: hunk.stack_id,
                start: hunk.start.saturating_add_signed(shift),
                lines: hunk.lines - (last_hunk.start + last_hunk.lines - hunk.start),
                line_shift: hunk.line_shift,
            },
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stack_simple() -> anyhow::Result<()> {
        let diff = InputDiff::try_from(
            "@@ -1,6 +1,7 @@
1
2
3
+4
5
6
7
",
        )?;
        let stack_ranges = &mut PathRanges::default();
        let stack_id = StackId::generate();
        let commit_id = git2::Oid::from_str("a")?;

        stack_ranges.add(stack_id, commit_id, vec![diff])?;

        let intersection = stack_ranges.intersection(4, 1);
        assert_eq!(intersection.len(), 1);

        Ok(())
    }

    #[test]
    fn stack_complex() -> anyhow::Result<()> {
        let diff_1 = InputDiff::try_from(
            "@@ -1,6 +1,7 @@
1
2
3
+4
5
6
7
",
        )?;
        let diff_2 = InputDiff::try_from(
            "@@ -2,6 +2,7 @@
2
3
4
+4.5
5
6
7
",
        )?;

        let stack_ranges = &mut PathRanges::default();
        let stack_id = StackId::generate();

        let commit_id = git2::Oid::from_str("a")?;
        stack_ranges.add(stack_id, commit_id, vec![diff_1])?;

        let commit_id = git2::Oid::from_str("b")?;
        stack_ranges.add(stack_id, commit_id, vec![diff_2])?;

        let intersection = stack_ranges.intersection(4, 1);
        assert_eq!(intersection.len(), 1);

        let intersection = stack_ranges.intersection(5, 1);
        assert_eq!(intersection.len(), 1);

        let intersection = stack_ranges.intersection(4, 2);
        assert_eq!(intersection.len(), 2);

        Ok(())
    }

    #[test]
    fn stack_basic_line_shift() -> anyhow::Result<()> {
        let diff_1 = InputDiff::try_from(
            "@@ -1,4 +1,5 @@
a
+b
a
a
a
",
        )?;
        let diff_2 = InputDiff::try_from(
            "@@ -1,3 +1,4 @@
+c
a
b
a
",
        )?;

        let stack_ranges = &mut PathRanges::default();
        let stack_id = StackId::generate();

        let commit_id = git2::Oid::from_str("a")?;
        stack_ranges.add(stack_id, commit_id, vec![diff_1])?;

        let commit_id = git2::Oid::from_str("b")?;
        stack_ranges.add(stack_id, commit_id, vec![diff_2])?;

        let result = stack_ranges.intersection(1, 1);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].commit_id, commit_id);

        Ok(())
    }

    #[test]
    fn stack_complex_line_shift() -> anyhow::Result<()> {
        let stack_ranges = &mut PathRanges::default();
        let stack_id = StackId::generate();

        let commit1_id = git2::Oid::from_str("a")?;
        let diff1 = InputDiff::try_from(
            "@@ -1,4 +1,5 @@
a
+b
a
a
a
",
        )?;
        stack_ranges.add(stack_id, commit1_id, vec![diff1])?;

        let commit2_id = git2::Oid::from_str("b")?;
        let diff2 = InputDiff::try_from(
            "@@ -1,3 +1,4 @@
+c
a
b
a
",
        )?;

        stack_ranges.add(stack_id, commit2_id, vec![diff2])?;

        let result = stack_ranges.intersection(1, 1);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].commit_id, commit2_id);

        let result = stack_ranges.intersection(2, 1);
        assert_eq!(result.len(), 0);

        let result = stack_ranges.intersection(3, 1);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].commit_id, commit1_id);

        Ok(())
    }

    #[test]
    fn stack_multiple_overwrites() -> anyhow::Result<()> {
        let stack_ranges = &mut PathRanges::default();
        let stack_id = StackId::generate();

        let commit1_id = git2::Oid::from_str("a")?;
        let diff_1 = InputDiff::try_from(
            "@@ -1,0 +1,7 @@
+a
+a
+a
+a
+a
+a
+a
",
        )?;
        stack_ranges.add(stack_id, commit1_id, vec![diff_1])?;

        let commit2_id = git2::Oid::from_str("b")?;
        let diff2 = InputDiff::try_from(
            "@@ -1,5 +1,5 @@
a
-a
+b
a
a
a
",
        )?;
        stack_ranges.add(stack_id, commit2_id, vec![diff2])?;

        let commit3_id = git2::Oid::from_str("c")?;
        let diff3 = InputDiff::try_from(
            "@@ -1,7 +1,7 @@
a
b
a
-a
+b
a
a
a
",
        )?;
        stack_ranges.add(stack_id, commit3_id, vec![diff3])?;

        let commit4_id = git2::Oid::from_str("d")?;
        let diff4 = InputDiff::try_from(
            "@@ -3,5 +3,5 @@
a
b
a
-a
+b
a
",
        )?;
        stack_ranges.add(stack_id, commit4_id, vec![diff4])?;

        let result = stack_ranges.intersection(1, 1);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].commit_id, commit1_id);

        let result = stack_ranges.intersection(2, 1);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].commit_id, commit2_id);

        let result = stack_ranges.intersection(4, 1);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].commit_id, commit3_id);

        let result = stack_ranges.intersection(6, 1);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].commit_id, commit4_id);

        Ok(())
    }

    #[test]
    fn stack_detect_deletion() -> anyhow::Result<()> {
        let stack_ranges = &mut PathRanges::default();
        let stack_id = StackId::generate();

        let commit1_id = git2::Oid::from_str("a")?;
        let diff_1 = InputDiff::try_from(
            "@@ -1,7 +1,6 @@
a
a
a
-a
a
a
a
",
        )?;
        stack_ranges.add(stack_id, commit1_id, vec![diff_1])?;

        let result = stack_ranges.intersection(3, 2);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].commit_id, commit1_id);

        Ok(())
    }

    #[test]
    fn stack_offset_and_split() -> anyhow::Result<()> {
        let stack_ranges = &mut PathRanges::default();
        let stack_id = StackId::generate();

        let commit1_id = git2::Oid::from_str("a")?;
        let diff_1 = InputDiff::try_from(
            "@@ -10,6 +10,9 @@
a
a
a
+b
+b
+b
a
a
a
",
        )?;
        stack_ranges.add(stack_id, commit1_id, vec![diff_1])?;

        let commit2_id = git2::Oid::from_str("b")?;
        let diff_2 = InputDiff::try_from(
            "@@ -1,6 +1,9 @@
a
a
a
+c
+c
+c
a
a
a
",
        )?;
        stack_ranges.add(stack_id, commit2_id, vec![diff_2])?;

        let commit3_id = git2::Oid::from_str("c")?;
        let diff_3 = InputDiff::try_from(
            "@@ -14,7 +14,7 @@
a
a
b
-b
+d
b
a
a
",
        )?;
        stack_ranges.add(stack_id, commit3_id, vec![diff_3])?;

        assert_eq!(stack_ranges.intersection(4, 3)[0].commit_id, commit2_id);
        assert_eq!(stack_ranges.intersection(15, 1).len(), 0);
        assert_eq!(stack_ranges.intersection(16, 1)[0].commit_id, commit1_id);
        assert_eq!(stack_ranges.intersection(17, 1)[0].commit_id, commit3_id);
        assert_eq!(stack_ranges.intersection(18, 1)[0].commit_id, commit1_id);
        assert_eq!(stack_ranges.intersection(19, 1).len(), 0);

        Ok(())
    }
}
