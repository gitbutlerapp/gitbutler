use gitbutler_stack::StackId;

#[derive(Debug, PartialEq, Clone)]
pub struct DependencyHunk {
    pub stack_id: StackId,
    pub commit_id: git2::Oid,
    pub start: i32,
    pub lines: i32,
    pub line_shift: i32,
}

impl DependencyHunk {
    fn end(&self) -> i32 {
        self.start + self.lines - 1
    }

    pub fn intersects(&self, start: i32, lines: i32) -> bool {
        self.end() >= start && self.start < start + lines
    }

    pub fn contains(&self, start: i32, lines: i32) -> bool {
        start > self.start && start + lines <= self.end()
    }
}
