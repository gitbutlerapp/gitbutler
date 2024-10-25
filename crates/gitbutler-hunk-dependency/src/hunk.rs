use gitbutler_stack::StackId;

/// A struct for tracking what stack and commit a hunk belongs to as its line numbers shift with
/// new changes come in from other commits and/or stacks.
#[derive(Debug, Clone, Copy)]
pub struct HunkRange {
    pub stack_id: StackId,
    pub commit_id: git2::Oid,
    pub start: u32,
    pub lines: u32,
    pub line_shift: i32,
}

impl HunkRange {
    pub fn intersects(&self, start: u32, lines: u32) -> bool {
        if self.lines == 0 {
            // Special case for checking if a point is inside a range, happens
            // when a change contains only deletions.
            return self.start >= start && self.start < start + lines;
        }
        self.start < start + lines && self.start + self.lines > start
    }

    pub fn contains(&self, start: u32, lines: u32) -> bool {
        start > self.start && start + lines <= self.start + self.lines
    }
}
