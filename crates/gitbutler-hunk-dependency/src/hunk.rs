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

        self.start <= (start + lines - 1) && (self.start + self.lines - 1) >= start
    }

    pub fn contains(&self, start: u32, lines: u32) -> bool {
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
        self.start > (start + lines - 1)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_deleted_file_intersects_everything() {
        let range = HunkRange {
            change_type: gitbutler_diff::ChangeType::Deleted,
            stack_id: StackId::generate(),
            commit_id: git2::Oid::from_str("a").unwrap(),
            start: 0,
            lines: 0,
            line_shift: 0,
        };

        assert!(range.intersects(1, 1));
        assert!(range.intersects(2, 2));
        assert!(range.intersects(1, 1));
        assert!(range.intersects(12, 10));
        assert!(range.intersects(4, 0));
        assert!(range.intersects(0, 0));
    }

    #[test]
    fn test_hunk_at_the_beginning() {
        let range = HunkRange {
            change_type: gitbutler_diff::ChangeType::Modified,
            stack_id: StackId::generate(),
            commit_id: git2::Oid::from_str("a").unwrap(),
            start: 1,
            lines: 10,
            line_shift: 0,
        };

        assert!(range.intersects(1, 1));
        assert!(range.intersects(1, 10));
        assert!(range.intersects(4, 2));
        assert!(range.intersects(10, 20));
        // Adding lines at the beginning of the file.
        assert!(!range.intersects(0, 0));

        assert!(!range.intersects(11, 20));
        assert!(!range.intersects(30, 1));
    }

    #[test]
    fn test_hunk_in_the_middle() {
        let range = HunkRange {
            change_type: gitbutler_diff::ChangeType::Modified,
            stack_id: StackId::generate(),
            commit_id: git2::Oid::from_str("a").unwrap(),
            start: 10,
            lines: 10,
            line_shift: 0,
        };

        assert!(range.intersects(1, 10));
        assert!(range.intersects(1, 20));
        assert!(range.intersects(1, 30));
        assert!(range.intersects(4, 10));
        assert!(range.intersects(10, 20));
        assert!(range.intersects(11, 20));
        assert!(range.intersects(15, 1));

        // Adding lines at the beginning of the file.
        assert!(!range.intersects(0, 0));

        assert!(!range.intersects(1, 1));
        assert!(!range.intersects(1, 9));
        assert!(!range.intersects(20, 1));
        assert!(!range.intersects(30, 1));
    }

    #[test]
    fn test_is_covered_by() {
        let range = HunkRange {
            change_type: gitbutler_diff::ChangeType::Modified,
            stack_id: StackId::generate(),
            commit_id: git2::Oid::from_str("a").unwrap(),
            start: 10,
            lines: 10,
            line_shift: 0,
        };

        assert!(range.covered_by(1, 20));
        assert!(range.covered_by(1, 30));
        assert!(range.covered_by(4, 16));
        assert!(range.covered_by(10, 20));
        // Adding lines at the beginning of the file.
        assert!(!range.covered_by(0, 0));

        assert!(!range.covered_by(10, 9));
        assert!(!range.covered_by(11, 20));
        assert!(!range.covered_by(15, 1));
        assert!(!range.covered_by(1, 1));
        assert!(!range.covered_by(1, 18));
        assert!(!range.covered_by(20, 1));
        assert!(!range.covered_by(30, 10));
    }

    #[test]
    fn test_contains() {
        let range = HunkRange {
            change_type: gitbutler_diff::ChangeType::Modified,
            stack_id: StackId::generate(),
            commit_id: git2::Oid::from_str("a").unwrap(),
            start: 10,
            lines: 10,
            line_shift: 0,
        };

        assert!(!range.contains(0, 0));
        assert!(!range.contains(1, 20));
        assert!(!range.contains(1, 30));
        assert!(!range.contains(4, 16));
        assert!(!range.contains(10, 20));
        assert!(!range.contains(10, 10));
        assert!(range.contains(11, 8));
        assert!(range.contains(11, 9));
    }

    #[test]
    fn test_follows() {
        let range = HunkRange {
            change_type: gitbutler_diff::ChangeType::Modified,
            stack_id: StackId::generate(),
            commit_id: git2::Oid::from_str("a").unwrap(),
            start: 10,
            lines: 10,
            line_shift: 0,
        };

        assert!(range.follows(0, 0));
        assert!(range.follows(1, 9));
        assert!(range.follows(9, 1));
        assert!(!range.follows(10, 1));
        assert!(!range.follows(11, 1));
        assert!(!range.follows(20, 1));
    }
}
