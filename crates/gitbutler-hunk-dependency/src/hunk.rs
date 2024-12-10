use gitbutler_stack::StackId;

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
    pub fn intersects(&self, start: u32, lines: u32) -> bool {
        if self.change_type == gitbutler_diff::ChangeType::Deleted {
            // Special case when file is deleted.
            return true;
        }

        if self.lines == 0 {
            // Special case when point is inside a range, happens
            // when a change contains only deletions.
            return self.start >= start && self.start < start + lines;
        }

        if start == 0 && lines == 0 {
            // Special case when adding lines at the top of the file.
            return false;
        }

        if lines == 0 {
            // Special case when only adding lines.
            return self.start <= start && self.start + self.lines > start;
        }

        self.start <= (start + lines - 1) && (self.start + self.lines - 1) >= start
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

    pub fn precedes(&self, start: u32) -> bool {
        (self.start + self.lines - 1) < start
    }

    pub fn follows(&self, start: u32, lines: u32) -> bool {
        if start == 0 && lines == 0 {
            // Special case when adding lines at the top of the file.
            return true;
        }

        if lines == 0 {
            // Special case when only adding lines.
            return self.start > start;
        }

        self.start > (start + lines - 1)
    }
}
