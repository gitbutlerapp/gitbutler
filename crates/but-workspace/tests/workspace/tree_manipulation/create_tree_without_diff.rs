use but_core::{Commit, DiffSpec, HunkHeader};
use but_testsupport::{CommandExt, git, visualize_tree};
use but_workspace::tree_manipulation::{ChangesSource, create_tree_without_diff};
use gix::prelude::ObjectIdExt;
use snapbox::IntoData;

use crate::utils::{CONTEXT_LINES, read_only_in_memory_scenario, writable_scenario};

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
    snapbox::assert_data_eq!(
        visualize_tree(actual_tree_id.attach(&repo)).to_string(),
        snapbox::str![[r#"
9c0554f
└── regular-change.txt:100644:35f45fd "base-1\nbase-2\nkeep-1\nkeep-2\n"

"#]]
        .raw()
    );
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
    snapbox::assert_data_eq!(
        visualize_tree(actual_tree_id.attach(&repo)).to_string(),
        snapbox::str![[r#"
4ce1de9
├── file:100644:8076ded "keep-a\nkeep-b\n"
└── regular-change.txt:100644:c01d3c5 "base-1\nbase-2\nkeep-1\ndrop-1\ndrop-2\nkeep-2\n"

"#]]
        .raw()
    );
    Ok(())
}

#[test]
fn removing_the_only_hunk_of_an_added_file_should_succeed() -> anyhow::Result<()> {
    // A commit that adds a file has no 'before' state for it, so removing that file's
    // only hunk cannot be expressed as a diff against anything. That used to be
    // rejected outright, telling the caller to use whole-file mode instead, which is
    // what made a hunk undraggable when it was a file's only change.
    let (repo, _tmp) = writable_scenario("plain-modifications");

    std::fs::write(
        repo.workdir().expect("non-bare repository").join("added"),
        "a\nb\n",
    )?;
    git(&repo).args(["add", "added"]).run();
    git(&repo)
        .args(["commit", "-m", "add a brand-new file"])
        .run();

    let commit_id = repo.rev_parse_single("HEAD")?.detach();

    let (actual_tree_id, dropped) = create_tree_without_diff(
        &repo,
        ChangesSource::Commit { id: commit_id },
        [DiffSpec {
            previous_path: None,
            path: "added".into(),
            hunk_headers: vec![HunkHeader {
                old_start: 1,
                old_lines: 0,
                new_start: 1,
                new_lines: 2,
            }],
        }],
        CONTEXT_LINES,
    )?;

    assert!(
        dropped.is_empty(),
        "the hunk covers the entire addition, so nothing is left unmatched"
    );
    // The added file is gone again, leaving the rest of the commit's tree untouched.
    snapbox::assert_data_eq!(
        visualize_tree(actual_tree_id.attach(&repo)).to_string(),
        snapbox::str![[r#"
db299ef
├── all-added:100644:e69de29 ""
├── all-modified:100644:f00c965 "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n"
└── all-removed:100644:f00c965 "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n"

"#]]
        .raw()
    );
    Ok(())
}

#[test]
fn removing_the_only_hunk_of_a_deleted_file_should_succeed() -> anyhow::Result<()> {
    // The mirror image of the addition: a commit that deletes a file has no 'after'
    // state for it, so removing that deletion's only hunk was rejected the same way.
    let (repo, _tmp) = writable_scenario("plain-modifications");
    let workdir = repo.workdir().expect("non-bare repository").to_owned();

    std::fs::write(workdir.join("doomed"), "a\nb\n")?;
    git(&repo).args(["add", "doomed"]).run();
    git(&repo).args(["commit", "-m", "add a file"]).run();
    git(&repo).args(["rm", "doomed"]).run();
    git(&repo).args(["commit", "-m", "delete it again"]).run();

    let commit_id = repo.rev_parse_single("HEAD")?.detach();

    let (actual_tree_id, dropped) = create_tree_without_diff(
        &repo,
        ChangesSource::Commit { id: commit_id },
        [DiffSpec {
            previous_path: None,
            path: "doomed".into(),
            hunk_headers: vec![HunkHeader {
                old_start: 1,
                old_lines: 2,
                new_start: 1,
                new_lines: 0,
            }],
        }],
        CONTEXT_LINES,
    )?;

    assert!(
        dropped.is_empty(),
        "the hunk covers the entire deletion, so nothing is left unmatched"
    );
    // The file is back with its original content, as if the commit never removed it.
    snapbox::assert_data_eq!(
        visualize_tree(actual_tree_id.attach(&repo)).to_string(),
        snapbox::str![[r#"
66c77b4
├── all-added:100644:e69de29 ""
├── all-modified:100644:f00c965 "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n"
├── all-removed:100644:f00c965 "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n"
└── doomed:100644:422c2b7 "a\nb\n"

"#]]
        .raw()
    );
    Ok(())
}

#[test]
fn an_unmatched_hunk_leaves_a_deletion_in_place() -> anyhow::Result<()> {
    // A spec that matches nothing is reported back rather than applied, so the file has
    // to stay deleted - bringing it back as an empty one would contradict the report.
    let (repo, _tmp) = writable_scenario("plain-modifications");
    let workdir = repo.workdir().expect("non-bare repository").to_owned();

    std::fs::write(workdir.join("doomed"), "a\nb\n")?;
    git(&repo).args(["add", "doomed"]).run();
    git(&repo).args(["commit", "-m", "add a file"]).run();
    git(&repo).args(["rm", "doomed"]).run();
    git(&repo).args(["commit", "-m", "delete it again"]).run();

    let commit_id = repo.rev_parse_single("HEAD")?.detach();

    let (actual_tree_id, dropped) = create_tree_without_diff(
        &repo,
        ChangesSource::Commit { id: commit_id },
        [DiffSpec {
            previous_path: None,
            path: "doomed".into(),
            hunk_headers: vec![HunkHeader {
                old_start: 99,
                old_lines: 1,
                new_start: 99,
                new_lines: 0,
            }],
        }],
        CONTEXT_LINES,
    )?;

    assert_eq!(
        dropped,
        vec![DiffSpec {
            previous_path: None,
            path: "doomed".into(),
            hunk_headers: vec![HunkHeader {
                old_start: 99,
                old_lines: 1,
                new_start: 99,
                new_lines: 0,
            }],
        }],
        "the unmatched hunk is reported back as it was requested"
    );
    // `doomed` is absent, exactly as the commit left it.
    snapbox::assert_data_eq!(
        visualize_tree(actual_tree_id.attach(&repo)).to_string(),
        snapbox::str![[r#"
db299ef
├── all-added:100644:e69de29 ""
├── all-modified:100644:f00c965 "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n"
└── all-removed:100644:f00c965 "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n"

"#]]
        .raw()
    );
    Ok(())
}

#[test]
fn an_unmatched_hunk_leaves_an_addition_in_place() -> anyhow::Result<()> {
    // An added *empty* file has no hunks at all, so any spec is unmatched. The addition
    // has to survive rather than being dropped from the tree.
    let (repo, _tmp) = writable_scenario("plain-modifications");

    std::fs::write(
        repo.workdir().expect("non-bare repository").join("empty"),
        "",
    )?;
    git(&repo).args(["add", "empty"]).run();
    git(&repo).args(["commit", "-m", "add an empty file"]).run();

    let commit_id = repo.rev_parse_single("HEAD")?.detach();

    let (actual_tree_id, dropped) = create_tree_without_diff(
        &repo,
        ChangesSource::Commit { id: commit_id },
        [DiffSpec {
            previous_path: None,
            path: "empty".into(),
            hunk_headers: vec![HunkHeader {
                old_start: 1,
                old_lines: 0,
                new_start: 1,
                new_lines: 1,
            }],
        }],
        CONTEXT_LINES,
    )?;

    assert_eq!(
        dropped,
        vec![DiffSpec {
            previous_path: None,
            path: "empty".into(),
            hunk_headers: vec![HunkHeader {
                old_start: 1,
                old_lines: 0,
                new_start: 1,
                new_lines: 1,
            }],
        }],
        "the unmatched hunk is reported back as it was requested"
    );
    // The empty file is still added, exactly as the commit left it.
    snapbox::assert_data_eq!(
        visualize_tree(actual_tree_id.attach(&repo)).to_string(),
        snapbox::str![[r#"
562ec11
├── all-added:100644:e69de29 ""
├── all-modified:100644:f00c965 "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n"
├── all-removed:100644:f00c965 "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n"
└── empty:100644:e69de29 ""

"#]]
        .raw()
    );
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
    snapbox::assert_data_eq!(
        err.to_string(),
        snapbox::str!["The source of changes cannot have a conflicted 'after' side."]
    );

    Ok(())
}
