use std::{path::PathBuf, str::FromStr};

use gitbutler_hunk_dependency::{diff::InputDiff, stack::StackHunkRanges};
use gitbutler_stack::StackId;

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
    let deps_stack = &mut StackHunkRanges::default();
    let stack_id = StackId::generate();
    let path = PathBuf::from_str("/test.txt")?;
    let commit_id = git2::Oid::from_str("a")?;

    deps_stack.add(stack_id, commit_id, &path, vec![diff]);

    let overlapping_commits = deps_stack.intersection(&path, 4, 1);
    assert_eq!(overlapping_commits.len(), 1);

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

    let deps_stack = &mut StackHunkRanges::default();
    let stack_id = StackId::generate();

    let path = PathBuf::from_str("/test.txt")?;
    let commit_id = git2::Oid::from_str("a")?;
    deps_stack.add(stack_id, commit_id, &path, vec![diff_1]);

    let commit_id = git2::Oid::from_str("b")?;
    deps_stack.add(stack_id, commit_id, &path, vec![diff_2]);

    let overlapping_commits = deps_stack.intersection(&path, 4, 1);
    assert_eq!(overlapping_commits.len(), 1);

    let overlapping_commits = deps_stack.intersection(&path, 5, 1);
    assert_eq!(overlapping_commits.len(), 1);

    let overlapping_commits = deps_stack.intersection(&path, 4, 2);
    assert_eq!(overlapping_commits.len(), 2);

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

    let deps_stack = &mut StackHunkRanges::default();
    let stack_id = StackId::generate();

    let path = PathBuf::from_str("/test.txt")?;
    let commit_id = git2::Oid::from_str("a")?;
    deps_stack.add(stack_id, commit_id, &path, vec![diff_1]);

    let commit_id = git2::Oid::from_str("b")?;
    deps_stack.add(stack_id, commit_id, &path, vec![diff_2]);

    let overlaps = deps_stack.intersection(&path, 1, 1);
    assert_eq!(overlaps.len(), 1);
    assert_eq!(overlaps[0].commit_id, commit_id);

    Ok(())
}

#[test]
fn stack_complex_line_shift() -> anyhow::Result<()> {
    let deps_stack = &mut StackHunkRanges::default();
    let stack_id = StackId::generate();
    let path = PathBuf::from_str("/test.txt")?;

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
    deps_stack.add(stack_id, commit1_id, &path, vec![diff1]);

    let commit2_id = git2::Oid::from_str("b")?;
    let diff2 = InputDiff::try_from(
        "@@ -1,3 +1,4 @@
+c
a
b
a
",
    )?;

    deps_stack.add(stack_id, commit2_id, &path, vec![diff2]);

    let result = deps_stack.intersection(&path, 1, 1);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].commit_id, commit2_id);

    let result = deps_stack.intersection(&path, 2, 1);
    assert_eq!(result.len(), 0);

    let result = deps_stack.intersection(&path, 3, 1);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].commit_id, commit1_id);

    Ok(())
}

#[test]
fn stack_multiple_overwrites() -> anyhow::Result<()> {
    let deps_stack = &mut StackHunkRanges::default();
    let stack_id = StackId::generate();
    let path = PathBuf::from_str("/test.txt")?;

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
    deps_stack.add(stack_id, commit1_id, &path, vec![diff_1]);

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
    deps_stack.add(stack_id, commit2_id, &path, vec![diff2]);

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
    deps_stack.add(stack_id, commit3_id, &path, vec![diff3]);

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
    deps_stack.add(stack_id, commit4_id, &path, vec![diff4]);

    let result = deps_stack.intersection(&path, 1, 1);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].commit_id, commit1_id);

    let result = deps_stack.intersection(&path, 2, 1);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].commit_id, commit2_id);

    let result = deps_stack.intersection(&path, 4, 1);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].commit_id, commit3_id);

    let result = deps_stack.intersection(&path, 6, 1);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].commit_id, commit4_id);

    Ok(())
}

#[test]
fn stack_detect_deletion() -> anyhow::Result<()> {
    let deps_stack = &mut StackHunkRanges::default();
    let stack_id = StackId::generate();
    let path = PathBuf::from_str("/test.txt")?;

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
    deps_stack.add(stack_id, commit1_id, &path, vec![diff_1]);

    let result = deps_stack.intersection(&path, 3, 2);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].commit_id, commit1_id);

    Ok(())
}
