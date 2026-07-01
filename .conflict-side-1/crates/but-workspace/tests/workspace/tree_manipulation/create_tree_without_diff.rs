use but_core::{Commit, DiffSpec, HunkHeader};
use but_testsupport::visualize_tree;
use but_workspace::tree_manipulation::{ChangesSource, create_tree_without_diff};
use gix::prelude::ObjectIdExt;

use crate::utils::{CONTEXT_LINES, read_only_in_memory_scenario};

#[test]
fn two_regular_commits_should_succeed() -> anyhow::Result<()> {
    let repo = read_only_in_memory_scenario("create_tree_without_diff-commit-sources")?;

    let changed_file = "regular-change.txt";
    let commit_id = repo.rev_parse_single("regular-source")?.detach();

    let (actual_tree_id, dropped) = create_tree_without_diff(
        &repo,
        ChangesSource::Commit { id: commit_id },
        [DiffSpec {
            previous_path: None,
            path: changed_file.into(),
            hunk_headers: vec![HunkHeader {
                old_start: 3,
                old_lines: 0,
                new_start: 4,
                new_lines: 2,
            }],
        }],
        CONTEXT_LINES,
    )?;

    assert!(dropped.is_empty());
    insta::assert_snapshot!(visualize_tree(actual_tree_id.attach(&repo)), @r#"
    9c0554f
    └── regular-change.txt:100644:35f45fd "base-1\nbase-2\nkeep-1\nkeep-2\n"
    "#);
    Ok(())
}

#[test]
fn conflicted_then_regular_should_succeed() -> anyhow::Result<()> {
    let repo = read_only_in_memory_scenario("create_tree_without_diff-commit-sources")?;

    let changed_file = "file";
    let commit_id = repo
        .rev_parse_single("conflicted-then-regular-source")?
        .detach();

    let (actual_tree_id, dropped) = create_tree_without_diff(
        &repo,
        ChangesSource::Commit { id: commit_id },
        [DiffSpec {
            previous_path: None,
            path: changed_file.into(),
            hunk_headers: vec![HunkHeader {
                old_start: 1,
                old_lines: 0,
                new_start: 2,
                new_lines: 2,
            }],
        }],
        CONTEXT_LINES,
    )?;

    assert!(dropped.is_empty());
    insta::assert_snapshot!(visualize_tree(actual_tree_id.attach(&repo)), @r#"
    4ce1de9
    ├── file:100644:8076ded "keep-a\nkeep-b\n"
    └── regular-change.txt:100644:c01d3c5 "base-1\nbase-2\nkeep-1\ndrop-1\ndrop-2\nkeep-2\n"
    "#);
    Ok(())
}

#[test]
fn regular_then_conflicted_should_bail() -> anyhow::Result<()> {
    let repo = read_only_in_memory_scenario("create_tree_without_diff-commit-sources")?;

    let commit_id = repo
        .rev_parse_single("regular-then-conflicted-source")?
        .detach();
    assert!(Commit::from_id(commit_id.attach(&repo))?.is_conflicted());

    let err = create_tree_without_diff(
        &repo,
        ChangesSource::Commit { id: commit_id },
        std::iter::empty::<DiffSpec>(),
        CONTEXT_LINES,
    )
    .unwrap_err();
    insta::assert_snapshot!(err.to_string(), @"The source of changes cannot have a conflicted 'after' side.");

    Ok(())
}
