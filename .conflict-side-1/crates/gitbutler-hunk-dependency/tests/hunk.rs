use gitbutler_hunk_dependency::HunkRange;
use gitbutler_stack::StackId;

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

    assert!(range.intersects(1, 1).unwrap());
    assert!(range.intersects(2, 2).unwrap());
    assert!(range.intersects(1, 1).unwrap());
    assert!(range.intersects(12, 10).unwrap());
    assert!(range.intersects(4, 0).unwrap());
    assert!(range.intersects(0, 0).unwrap());
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

    assert!(range.intersects(1, 1).unwrap());
    assert!(range.intersects(1, 10).unwrap());
    assert!(range.intersects(4, 2).unwrap());
    assert!(range.intersects(10, 20).unwrap());
    assert!(range.intersects(4, 0).unwrap());
    // Adding lines at the beginning of the file.
    assert!(!range.intersects(0, 0).unwrap());

    assert!(!range.intersects(11, 20).unwrap());
    assert!(!range.intersects(30, 1).unwrap());
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

    assert!(range.intersects(1, 10).unwrap());
    assert!(range.intersects(1, 20).unwrap());
    assert!(range.intersects(1, 30).unwrap());
    assert!(range.intersects(4, 10).unwrap());
    assert!(range.intersects(19, 0).unwrap());
    assert!(range.intersects(10, 0).unwrap());
    assert!(range.intersects(10, 10).unwrap());
    assert!(range.intersects(10, 20).unwrap());
    assert!(range.intersects(11, 20).unwrap());
    assert!(range.intersects(15, 1).unwrap());

    // Adding lines at the beginning of the file.
    assert!(!range.intersects(0, 0).unwrap());

    assert!(!range.intersects(20, 0).unwrap());
    assert!(!range.intersects(1, 1).unwrap());
    assert!(!range.intersects(1, 9).unwrap());
    assert!(!range.intersects(20, 1).unwrap());
    assert!(!range.intersects(30, 1).unwrap());
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
    assert!(!range.contains(19, 0));
    assert!(range.contains(11, 8));
    assert!(range.contains(11, 9));
    assert!(range.contains(10, 0));
    assert!(range.contains(18, 0));
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

    assert!(range.follows(0, 0).unwrap());
    assert!(range.follows(1, 9).unwrap());
    assert!(range.follows(9, 1).unwrap());
    assert!(!range.follows(10, 0).unwrap());
    assert!(!range.follows(11, 0).unwrap());
    assert!(!range.follows(10, 1).unwrap());
    assert!(!range.follows(11, 1).unwrap());
    assert!(!range.follows(20, 1).unwrap());
}
