use but_testsupport::assure_stable_env;
use but_workspace::{DiffSpec, commit_engine};
use commit_engine::Destination;
use gix::prelude::ObjectIdExt;

use crate::utils::{
    CONTEXT_LINES, commit_from_outcome, commit_whole_files_and_all_hunks_from_workspace, diff_spec,
    hunk_header, read_only_in_memory_scenario, to_change_specs_all_hunks_with_context_lines,
    to_change_specs_whole_file, visualize_tree, writable_scenario, writable_scenario_with_ssh_key,
    write_sequence,
};

mod with_refs_update {}

#[test]
fn from_unborn_head() -> anyhow::Result<()> {
    assure_stable_env();

    let (repo, _tmp) = writable_scenario("unborn-untracked");
    let outcome = commit_whole_files_and_all_hunks_from_workspace(
        &repo,
        Destination::NewCommit {
            parent_commit_id: None,
            message: "the commit message".into(),
            stack_segment: None,
        },
    )?;
    insta::assert_debug_snapshot!(&outcome, @r"
    CreateCommitOutcome {
        rejected_specs: [],
        new_commit: Some(
            Sha1(b7cb7efa62f48dfc60e4db44837121d3b3eab4e0),
        ),
        changed_tree_pre_cherry_pick: Some(
            Sha1(861d6e23ee6a2d7276618bb78700354a3506bd71),
        ),
        references: [],
        rebase_output: None,
        index: None,
    }
    ");

    let new_commit_id = outcome.new_commit.expect("a new commit was created");
    assert!(
        repo.try_find_reference(repo.head_name()?.expect("not detached").as_ref())?
            .is_none(),
        "the HEAD reference isn't altered, so the repository stays unborn",
    );

    let new_commit = new_commit_id.attach(&repo).object()?.peel_to_commit()?;
    assert_eq!(new_commit.message_raw()?, "the commit message");

    let tree = visualize_tree(&repo, &outcome)?;
    insta::assert_snapshot!(tree, @r#"
    861d6e2
    └── not-yet-tracked:100644:d95f3ad "content\n"
    "#);

    std::fs::write(
        repo.workdir_path("new-untracked").expect("non-bare"),
        "new-content",
    )?;
    let outcome = commit_whole_files_and_all_hunks_from_workspace(
        &repo,
        Destination::NewCommit {
            parent_commit_id: Some(new_commit_id),
            message: "the second commit".into(),
            stack_segment: None,
        },
    )?;

    insta::assert_debug_snapshot!(&outcome, @r"
    CreateCommitOutcome {
        rejected_specs: [],
        new_commit: Some(
            Sha1(b7cd2309cbac81a85596b3e39756230943cfd8e5),
        ),
        changed_tree_pre_cherry_pick: Some(
            Sha1(a0044697412bfa8432298d6bd6a2ad0dbd655c9f),
        ),
        references: [],
        rebase_output: None,
        index: None,
    }
    ");
    let tree = visualize_tree(&repo, &outcome)?;
    insta::assert_snapshot!(tree, @r#"
    a004469
    ├── new-untracked:100644:72278a7 "new-content"
    └── not-yet-tracked:100644:d95f3ad "content\n"
    "#);
    Ok(())
}

#[test]
fn from_unborn_head_with_selection() -> anyhow::Result<()> {
    assure_stable_env();

    let (repo, _tmp) = writable_scenario("unborn-untracked");
    let destination = Destination::NewCommit {
        parent_commit_id: None,
        message: "the commit with selection".into(),
        stack_segment: None,
    };
    let outcome = commit_engine::create_commit(
        &repo,
        destination,
        None,
        vec![DiffSpec {
            previous_path: None,
            path: "not-yet-tracked".into(),
            hunk_headers: vec![hunk_header("-1,0", "+1,1")],
        }],
        CONTEXT_LINES,
    )?;

    let tree = visualize_tree(&repo, &outcome)?;
    insta::assert_snapshot!(tree, @r#"
    861d6e2
    └── not-yet-tracked:100644:d95f3ad "content\n"
    "#);

    write_sequence(&repo, "also-untracked", [(1, 10)])?;
    let destination = Destination::NewCommit {
        parent_commit_id: outcome.new_commit,
        message: "the commit with sub-selection".into(),
        stack_segment: None,
    };
    let outcome = commit_engine::create_commit(
        &repo,
        destination.clone(),
        None,
        vec![DiffSpec {
            previous_path: None,
            path: "also-untracked".into(),
            // Take 3 lines in the middle, instead of 10
            hunk_headers: vec![hunk_header("-0,0", "+4,3")],
        }],
        CONTEXT_LINES,
    )?;
    assert_eq!(
        outcome.rejected_specs,
        [],
        "hunk-ranges can also be applied"
    );

    let tree = visualize_tree(&repo, &outcome)?;
    insta::assert_snapshot!(tree, @r#"
    fe03a86
    ├── also-untracked:100644:4578bc1 "4\n5\n6\n"
    └── not-yet-tracked:100644:d95f3ad "content\n"
    "#);

    let outcome = commit_engine::create_commit(
        &repo,
        destination,
        None,
        vec![DiffSpec {
            previous_path: None,
            path: "also-untracked".into(),
            // Take 3 lines in the middle, instead of 10, but line by line like the UI would select it.
            hunk_headers: vec![
                hunk_header("-0,0", "+4,1"),
                hunk_header("-0,0", "+5,1"),
                hunk_header("-0,0", "+6,1"),
            ],
        }],
        CONTEXT_LINES,
    )?;
    assert_eq!(
        outcome.rejected_specs,
        [],
        "hunk-ranges can also be applied"
    );

    let tree = visualize_tree(&repo, &outcome)?;
    insta::assert_snapshot!(tree, @r#"
    fe03a86
    ├── also-untracked:100644:4578bc1 "4\n5\n6\n"
    └── not-yet-tracked:100644:d95f3ad "content\n"
    "#);
    Ok(())
}

#[test]
#[cfg(unix)]
fn from_unborn_head_all_file_types() -> anyhow::Result<()> {
    assure_stable_env();

    let repo = read_only_in_memory_scenario("unborn-untracked-all-file-types")?;
    let new_commit_from_unborn = Destination::NewCommit {
        parent_commit_id: None,
        message: "the commit message".into(),
        stack_segment: None,
    };
    let outcome =
        commit_whole_files_and_all_hunks_from_workspace(&repo, new_commit_from_unborn.clone())?;

    assert_eq!(
        outcome.rejected_specs,
        Vec::new(),
        "everything was committed"
    );
    let new_commit_id = outcome.new_commit.expect("a new commit was created");

    let new_commit = new_commit_id.attach(&repo).object()?.peel_to_commit()?;
    assert_eq!(new_commit.message_raw()?, "the commit message");

    let tree = visualize_tree(&repo, &outcome)?;
    insta::assert_snapshot!(tree, @r#"
    7f802e9
    ├── link:120000:faf96c1 "untracked"
    ├── untracked:100644:d95f3ad "content\n"
    └── untracked-exe:100755:86daf54 "exe\n"
    "#);

    let outcome = commit_engine::create_commit(
        &repo,
        new_commit_from_unborn,
        None,
        vec![diff_spec(None, "link", Some(hunk_header("-1,0", "+1,1")))],
        CONTEXT_LINES,
    )?;
    // The link points to the untracked file.
    let tree = visualize_tree(&repo, &outcome)?;
    insta::assert_snapshot!(tree, @r#"
    654694d
    └── link:120000:faf96c1 "untracked"
    "#);

    Ok(())
}

#[test]
#[cfg(unix)]
fn from_first_commit_all_file_types_changed() -> anyhow::Result<()> {
    assure_stable_env();

    let repo = read_only_in_memory_scenario("all-file-types-changed")?;
    let outcome = commit_whole_files_and_all_hunks_from_workspace(
        &repo,
        Destination::NewCommit {
            parent_commit_id: Some(repo.rev_parse_single("HEAD")?.into()),
            message: "the commit message".into(),
            stack_segment: None,
        },
    )?;
    assert_eq!(outcome.rejected_specs, []);

    let tree = visualize_tree(&repo, &outcome)?;
    insta::assert_snapshot!(tree, @r#"
    9be09ac
    ├── soon-executable:100755:d95f3ad "content\n"
    ├── soon-file-not-link:100644:72f007b "ordinary content\n"
    └── soon-not-executable:100644:86daf54 "exe\n"
    "#);
    Ok(())
}

#[test]
fn unborn_with_added_submodules() -> anyhow::Result<()> {
    assure_stable_env();

    let (repo, _tmp) = writable_scenario("unborn-with-submodules");
    let worktree_changes = but_core::diff::worktree_changes(&repo)?;
    let outcome = commit_engine::create_commit(
        &repo,
        Destination::NewCommit {
            parent_commit_id: None,
            message:
                "submodules have to be given as whole files but can then be handled correctly \
            (but without Git's special handling)"
                    .into(),
            stack_segment: None,
        },
        None,
        to_change_specs_whole_file(worktree_changes),
        CONTEXT_LINES,
    )?;

    assert_eq!(
        outcome.rejected_specs,
        vec![],
        "Everything could be added to the repository"
    );
    let tree = visualize_tree(&repo, &outcome)?;
    insta::assert_snapshot!(tree, @r#"
    6260c86
    ├── .gitmodules:100644:49dc605 "[submodule \"m1\"]\n\tpath = m1\n\turl = ./module\n"
    ├── m1:160000:a047f81 
    └── module:160000:a047f81
    "#);
    Ok(())
}

#[test]
fn deletions() -> anyhow::Result<()> {
    assure_stable_env();

    let repo = read_only_in_memory_scenario("delete-all-file-types")?;
    let head_commit = repo.rev_parse_single("HEAD")?;
    insta::assert_snapshot!(but_testsupport::visualize_tree(head_commit.object()?.peel_to_tree()?.id()), @r#"
    cecc2da
    ├── .gitmodules:100644:51f8807 "[submodule \"submodule\"]\n\tpath = submodule\n\turl = ./embedded-repository\n"
    ├── embedded-repository:160000:a047f81 
    ├── executable:100755:86daf54 "exe\n"
    ├── file-to-remain:100644:d95f3ad "content\n"
    ├── link:120000:b158162 "file-to-remain"
    └── submodule:160000:a047f81
    "#);
    let new_commit_from_deletions = Destination::NewCommit {
        parent_commit_id: Some(head_commit.into()),
        message: "deletions maybe a bit special".into(),
        stack_segment: None,
    };
    let outcome =
        commit_whole_files_and_all_hunks_from_workspace(&repo, new_commit_from_deletions.clone())?;

    insta::assert_snapshot!(visualize_tree(&repo, &outcome)?, @r#"
    c15318d
    └── file-to-remain:100644:d95f3ad "content\n"
    "#);
    assert_eq!(
        but_core::diff::worktree_changes(&repo)?.changes.len(),
        5,
        "we don't actually change the index to match, nor is the HEAD changed, worktree changes seem to remain"
    );

    let outcome = commit_engine::create_commit(
        &repo,
        new_commit_from_deletions,
        None,
        // Pass the link with hunks that indicate a line deletion.
        vec![diff_spec(None, "link", Some(hunk_header("-1,1", "+1,0")))],
        CONTEXT_LINES,
    )?;
    // And the link got deleted.
    insta::assert_snapshot!(visualize_tree(&repo, &outcome)?, @r#"
    921b85b
    ├── .gitmodules:100644:51f8807 "[submodule \"submodule\"]\n\tpath = submodule\n\turl = ./embedded-repository\n"
    ├── embedded-repository:160000:a047f81 
    ├── executable:100755:86daf54 "exe\n"
    ├── file-to-remain:100644:d95f3ad "content\n"
    └── submodule:160000:a047f81
    "#);

    Ok(())
}

#[test]
fn modifications() -> anyhow::Result<()> {
    assure_stable_env();

    let repo = read_only_in_memory_scenario("all-file-types-modified")?;
    let head_commit = repo.rev_parse_single("HEAD")?;
    insta::assert_snapshot!(but_testsupport::visualize_tree(head_commit.object()?.peel_to_tree()?.id()), @r#"
    3fd29f0
    ├── executable:100755:01e79c3 "1\n2\n3\n"
    ├── file:100644:3aac70f "5\n6\n7\n8\n"
    └── link:120000:c4c364c "nonexisting-target"
    "#);
    let new_commit_from_rename = Destination::NewCommit {
        parent_commit_id: Some(head_commit.into()),
        message: "modifications of content and symlinks".into(),
        stack_segment: None,
    };
    let outcome =
        commit_whole_files_and_all_hunks_from_workspace(&repo, new_commit_from_rename.clone())?;

    insta::assert_snapshot!(visualize_tree(&repo, &outcome)?, @r#"
    db51146
    ├── executable:100755:8a1218a "1\n2\n3\n4\n5\n"
    ├── file:100644:c5c4315 "5\n6\n7\n8\n9\n10\n"
    └── link:120000:94e4e07 "other-nonexisting-target"
    "#);

    let outcome = commit_engine::create_commit(
        &repo,
        new_commit_from_rename,
        None,
        // Pass the link with hunks that indicate a modification.
        vec![diff_spec(None, "link", Some(hunk_header("-1,1", "+1,1")))],
        CONTEXT_LINES,
    )?;
    insta::assert_snapshot!(visualize_tree(&repo, &outcome)?, @r#"
    f10dc60
    ├── executable:100755:01e79c3 "1\n2\n3\n"
    ├── file:100644:3aac70f "5\n6\n7\n8\n"
    └── link:120000:94e4e07 "other-nonexisting-target"
    "#);
    Ok(())
}

#[test]
fn renames() -> anyhow::Result<()> {
    assure_stable_env();

    let repo = read_only_in_memory_scenario("all-file-types-renamed-and-modified")?;
    let head_commit = repo.rev_parse_single("HEAD")?;
    insta::assert_snapshot!(but_testsupport::visualize_tree(head_commit.object()?.peel_to_tree()?.id()), @r#"
    3fd29f0
    ├── executable:100755:01e79c3 "1\n2\n3\n"
    ├── file:100644:3aac70f "5\n6\n7\n8\n"
    └── link:120000:c4c364c "nonexisting-target"
    "#);
    let new_commit_from_rename = Destination::NewCommit {
        parent_commit_id: Some(head_commit.into()),
        message: "renames need special care to delete the source".into(),
        stack_segment: None,
    };
    let outcome =
        commit_whole_files_and_all_hunks_from_workspace(&repo, new_commit_from_rename.clone())?;

    insta::assert_snapshot!(visualize_tree(&repo, &outcome)?, @r#"
    e56fc9b
    ├── executable-renamed:100755:8a1218a "1\n2\n3\n4\n5\n"
    ├── file-renamed:100644:c5c4315 "5\n6\n7\n8\n9\n10\n"
    └── link-renamed:120000:94e4e07 "other-nonexisting-target"
    "#);

    insta::assert_debug_snapshot!(
        utils::worktree_change_diffs(&repo, 0)?, @r#"
    [
        (
            Some(
                "executable",
            ),
            "executable-renamed",
            Patch {
                hunks: [
                    DiffHunk("@@ -4,0 +4,2 @@
                    +4
                    +5
                    "),
                ],
                is_result_of_binary_to_text_conversion: false,
                lines_added: 2,
                lines_removed: 0,
            },
        ),
        (
            Some(
                "file",
            ),
            "file-renamed",
            Patch {
                hunks: [
                    DiffHunk("@@ -5,0 +5,2 @@
                    +9
                    +10
                    "),
                ],
                is_result_of_binary_to_text_conversion: false,
                lines_added: 2,
                lines_removed: 0,
            },
        ),
        (
            None,
            "link",
            Patch {
                hunks: [
                    DiffHunk("@@ -1,1 +1,0 @@
                    -nonexisting-target
                    "),
                ],
                is_result_of_binary_to_text_conversion: false,
                lines_added: 0,
                lines_removed: 1,
            },
        ),
        (
            None,
            "link-renamed",
            Patch {
                hunks: [
                    DiffHunk("@@ -1,0 +1,1 @@
                    +other-nonexisting-target
                    "),
                ],
                is_result_of_binary_to_text_conversion: false,
                lines_added: 1,
                lines_removed: 0,
            },
        ),
    ]
    "#);
    let outcome = commit_engine::create_commit(
        &repo,
        new_commit_from_rename,
        None,
        // Links are never considered renamed, so this is only the addition part.
        vec![diff_spec(
            None,
            "link-renamed",
            Some(hunk_header("-1,0", "+1,1")),
        )],
        CONTEXT_LINES,
    )?;
    insta::assert_snapshot!(visualize_tree(&repo, &outcome)?, @r#"
    7accf8d
    ├── executable:100755:01e79c3 "1\n2\n3\n"
    ├── file:100644:3aac70f "5\n6\n7\n8\n"
    ├── link:120000:c4c364c "nonexisting-target"
    └── link-renamed:120000:94e4e07 "other-nonexisting-target"
    "#);
    Ok(())
}

#[test]
fn renames_with_selections() -> anyhow::Result<()> {
    assure_stable_env();

    let repo = read_only_in_memory_scenario("all-file-types-renamed-and-modified")?;
    let head_commit_id = repo.rev_parse_single("HEAD")?;
    insta::assert_snapshot!(but_testsupport::visualize_tree(head_commit_id.object()?.peel_to_tree()?.id()), @r#"
    3fd29f0
    ├── executable:100755:01e79c3 "1\n2\n3\n"
    ├── file:100644:3aac70f "5\n6\n7\n8\n"
    └── link:120000:c4c364c "nonexisting-target"
    "#);
    insta::assert_debug_snapshot!(
        utils::worktree_change_diffs(&repo, 0)?,
        @r#"
    [
        (
            Some(
                "executable",
            ),
            "executable-renamed",
            Patch {
                hunks: [
                    DiffHunk("@@ -4,0 +4,2 @@
                    +4
                    +5
                    "),
                ],
                is_result_of_binary_to_text_conversion: false,
                lines_added: 2,
                lines_removed: 0,
            },
        ),
        (
            Some(
                "file",
            ),
            "file-renamed",
            Patch {
                hunks: [
                    DiffHunk("@@ -5,0 +5,2 @@
                    +9
                    +10
                    "),
                ],
                is_result_of_binary_to_text_conversion: false,
                lines_added: 2,
                lines_removed: 0,
            },
        ),
        (
            None,
            "link",
            Patch {
                hunks: [
                    DiffHunk("@@ -1,1 +1,0 @@
                    -nonexisting-target
                    "),
                ],
                is_result_of_binary_to_text_conversion: false,
                lines_added: 0,
                lines_removed: 1,
            },
        ),
        (
            None,
            "link-renamed",
            Patch {
                hunks: [
                    DiffHunk("@@ -1,0 +1,1 @@
                    +other-nonexisting-target
                    "),
                ],
                is_result_of_binary_to_text_conversion: false,
                lines_added: 1,
                lines_removed: 0,
            },
        ),
    ]
    "#
    );

    let outcome = commit_engine::create_commit(
        &repo,
        Destination::NewCommit {
            parent_commit_id: Some(head_commit_id.into()),
            message: "renames need special care to delete the source, even with selection".into(),
            stack_segment: None,
        },
        None,
        vec![
            diff_spec(
                Some("executable"),
                "executable-renamed",
                Some(
                    // Context lines can't be selected, so select first of new here.
                    // Old is also the anchor here.
                    hunk_header("-0,0", "+4,1"),
                ),
            ),
            diff_spec(
                Some("file"),
                "file-renamed",
                Some(
                    // Keep only the last line.
                    hunk_header("-0,0", "+6,1"),
                ),
            ),
            // delete the source of the link, selections don't apply, and we don't want to see it.
            diff_spec(None, "link", None),
        ],
        UI_CONTEXT_LINES,
    )?;
    assert_eq!(outcome.rejected_specs, [], "everything was assigned");

    insta::assert_snapshot!(visualize_tree(&repo, &outcome)?, @r#"
    e47440e
    ├── executable-renamed:100755:94ebaf9 "1\n2\n3\n4\n"
    └── file-renamed:100644:76cf35b "5\n6\n7\n8\n10\n"
    "#);
    Ok(())
}

#[test]
fn modification_with_complex_selection() -> anyhow::Result<()> {
    assure_stable_env();

    let repo = read_only_in_memory_scenario("plain-modifications")?;
    insta::assert_snapshot!(but_testsupport::visualize_tree(repo.head_tree_id()?), @r#"
    db299ef
    ├── all-added:100644:e69de29 ""
    ├── all-modified:100644:f00c965 "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n"
    └── all-removed:100644:f00c965 "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n"
    "#);

    insta::assert_debug_snapshot!(
        utils::worktree_change_diffs(&repo, 0)?[1], @r#"
    (
        None,
        "all-modified",
        Patch {
            hunks: [
                DiffHunk("@@ -1,10 +1,10 @@
                -1
                -2
                -3
                -4
                -5
                -6
                -7
                -8
                -9
                -10
                +11
                +12
                +13
                +14
                +15
                +16
                +17
                +18
                +19
                +20
                "),
            ],
            is_result_of_binary_to_text_conversion: false,
            lines_added: 10,
            lines_removed: 10,
        },
    )
    "#);

    let outcome = commit_engine::create_commit(
        &repo,
        Destination::NewCommit {
            parent_commit_id: Some(repo.head_id()?.into()),
            message: "commit only the modified file with a complex selection".into(),
            stack_segment: None,
        },
        None,
        vec![diff_spec(
            None,
            "all-modified",
            [
                // commit NOT '2,3' of the old
                hunk_header("-2,2", "+0,0"),
                // commit NOT '6,7' of the old
                hunk_header("-6,2", "+0,0"),
                // commit NOT '9' of the old
                hunk_header("-9,1", "+0,0"),
                // commit NOT '10' of the old
                hunk_header("-10,1", "+0,0"),
                // commit '11' of the new
                hunk_header("-0,0", "+1,1"),
                // commit '15,16' of the new
                hunk_header("-0,0", "+5,2"),
                // commit '19,20' of the new
                hunk_header("-0,0", "+9,2"),
            ],
        )],
        UI_CONTEXT_LINES,
    )?;
    assert_eq!(outcome.rejected_specs, [], "everything was assigned");

    insta::assert_snapshot!(visualize_tree(&repo, &outcome)?, @r#"
    4bbd0d5
    ├── all-added:100644:e69de29 ""
    ├── all-modified:100644:fcf7eb0 "1\n4\n5\n8\n11\n15\n16\n19\n20\n"
    └── all-removed:100644:f00c965 "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n"
    "#);

    let outcome = commit_engine::create_commit(
        &repo,
        Destination::NewCommit {
            parent_commit_id: Some(repo.head_id()?.into()),
            message: "like before, but select individual lines like the UI would".into(),
            stack_segment: None,
        },
        None,
        vec![diff_spec(
            None,
            "all-modified",
            [
                // commit NOT '2,3' of the old
                hunk_header("-2,1", "+0,0"),
                hunk_header("-3,1", "+0,0"),
                // commit NOT '6,7' of the old
                hunk_header("-6,1", "+0,0"),
                hunk_header("-7,1", "+0,0"),
                // commit NOT '9' of the old
                hunk_header("-9,1", "+0,0"),
                // commit NOT '10' of the old
                hunk_header("-10,1", "+0,0"),
                // commit '11' of the new
                hunk_header("-0,0", "+1,1"),
                // commit '15,16' of the new
                hunk_header("-0,0", "+5,1"),
                hunk_header("-0,0", "+6,1"),
                // commit '19,20' of the new
                hunk_header("-0,0", "+9,1"),
                hunk_header("-0,0", "+10,1"),
            ],
        )],
        UI_CONTEXT_LINES,
    )?;
    assert_eq!(outcome.rejected_specs, [], "everything was assigned");

    insta::assert_snapshot!(visualize_tree(&repo, &outcome)?, @r#"
    4bbd0d5
    ├── all-added:100644:e69de29 ""
    ├── all-modified:100644:fcf7eb0 "1\n4\n5\n8\n11\n15\n16\n19\n20\n"
    └── all-removed:100644:f00c965 "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n"
    "#);
    Ok(())
}

#[test]
fn submodule_typechanges() -> anyhow::Result<()> {
    assure_stable_env();

    let (repo, _tmp) = writable_scenario("submodule-typechanges");
    let worktree_changes = but_core::diff::worktree_changes(&repo)?;
    insta::assert_debug_snapshot!(worktree_changes.changes, @r#"
    [
        TreeChange {
            path: ".gitmodules",
            status: Modification {
                previous_state: ChangeState {
                    id: Sha1(51f8807c330e4ae8643ca943231cc6e176038aca),
                    kind: Blob,
                },
                state: ChangeState {
                    id: Sha1(57fc33bc66d69e4df4ab23c33ae1101e67e56079),
                    kind: Blob,
                },
                flags: None,
            },
        },
        TreeChange {
            path: "file",
            status: Modification {
                previous_state: ChangeState {
                    id: Sha1(d95f3ad14dee633a758d2e331151e950dd13e4ed),
                    kind: Blob,
                },
                state: ChangeState {
                    id: Sha1(a047f8183ba2bb7eb00ef89e60050c5fde740483),
                    kind: Commit,
                },
                flags: Some(
                    TypeChange,
                ),
            },
        },
        TreeChange {
            path: "submodule",
            status: Modification {
                previous_state: ChangeState {
                    id: Sha1(a047f8183ba2bb7eb00ef89e60050c5fde740483),
                    kind: Commit,
                },
                state: ChangeState {
                    id: Sha1(d95f3ad14dee633a758d2e331151e950dd13e4ed),
                    kind: Blob,
                },
                flags: Some(
                    TypeChange,
                ),
            },
        },
    ]
    "#);
    let outcome = commit_engine::create_commit(
        &repo,
        Destination::NewCommit {
            parent_commit_id: Some(repo.rev_parse_single("HEAD")?.into()),
            message:
                "submodules have to be given as whole files but can then be handled correctly \
            (but without Git's special handling)"
                    .into(),
            stack_segment: None,
        },
        None,
        to_change_specs_whole_file(worktree_changes),
        CONTEXT_LINES,
    )?;

    assert_eq!(
        outcome.rejected_specs,
        vec![],
        "Everything could be added to the repository"
    );
    let tree = visualize_tree(&repo, &outcome)?;
    insta::assert_snapshot!(tree, @r#"
    05b8ed2
    ├── .gitmodules:100644:57fc33b "[submodule \"submodule\"]\n\tpath = file\n\turl = ./embedded-repository\n"
    ├── embedded-repository:160000:a047f81 
    ├── file:160000:a047f81 
    └── submodule:100644:d95f3ad "content\n"
    "#);
    Ok(())
}

#[test]
fn commit_to_one_below_tip() -> anyhow::Result<()> {
    assure_stable_env();

    let (repo, _tmp) = writable_scenario("two-commits-with-line-offset");
    // Repeat the file, but replace the last 20 lines with 30-50
    write_sequence(&repo, "file", [(20, Some(40)), (80, None), (30, Some(50))])?;
    let first_commit = Destination::NewCommit {
        parent_commit_id: Some(repo.rev_parse_single("first-commit")?.into()),
        message: "we apply a change with line offsets on top of the first commit, so a cherry-pick is necessary.".into(),
        stack_segment: None,
    };

    let outcome = commit_whole_files_and_all_hunks_from_workspace(&repo, first_commit)?;
    assert_eq!(outcome.rejected_specs, vec![], "nothing was rejected");
    let tree = visualize_tree(&repo, &outcome)?;
    insta::assert_snapshot!(tree, @r#"
    754a70c
    └── file:100644:cc418b0 "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n11\n12\n13\n14\n15\n16\n17\n18\n19\n20\n21\n22\n23\n24\n25\n26\n27\n28\n29\n30\n31\n32\n33\n34\n35\n36\n37\n38\n39\n40\n41\n42\n43\n44\n45\n46\n47\n48\n49\n50\n51\n52\n53\n54\n55\n56\n57\n58\n59\n60\n61\n62\n63\n64\n65\n66\n67\n68\n69\n70\n71\n72\n73\n74\n75\n76\n77\n78\n79\n80\n30\n31\n32\n33\n34\n35\n36\n37\n38\n39\n40\n41\n42\n43\n44\n45\n46\n47\n48\n49\n50\n"
    "#);
    Ok(())
}

#[test]
fn commit_to_one_below_tip_with_three_context_lines() -> anyhow::Result<()> {
    assure_stable_env();

    let (repo, _tmp) = writable_scenario("two-commits-with-line-offset");
    write_sequence(&repo, "file", [(20, Some(40)), (80, None), (30, Some(50))])?;
    for context_lines in [0, 3, 5] {
        let first_commit = Destination::NewCommit {
            parent_commit_id: Some(repo.rev_parse_single("first-commit")?.into()),
            message: "When using context lines, we'd still think this works just like before"
                .into(),
            stack_segment: None,
        };

        let outcome = commit_engine::create_commit(
            &repo,
            first_commit,
            None,
            to_change_specs_all_hunks_with_context_lines(
                &repo,
                but_core::diff::worktree_changes(&repo)?,
                context_lines,
            )?,
            context_lines,
        )?;

        assert_eq!(
            outcome.new_commit.map(|id| id.to_string()),
            Some("215719d87875599c931d04a6e9b87dd2b6ef9885".to_string())
        );
        let tree = visualize_tree(&repo, &outcome)?;
        assert_eq!(
            tree,
            r#"754a70c
└── file:100644:cc418b0 "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n11\n12\n13\n14\n15\n16\n17\n18\n19\n20\n21\n22\n23\n24\n25\n26\n27\n28\n29\n30\n31\n32\n33\n34\n35\n36\n37\n38\n39\n40\n41\n42\n43\n44\n45\n46\n47\n48\n49\n50\n51\n52\n53\n54\n55\n56\n57\n58\n59\n60\n61\n62\n63\n64\n65\n66\n67\n68\n69\n70\n71\n72\n73\n74\n75\n76\n77\n78\n79\n80\n30\n31\n32\n33\n34\n35\n36\n37\n38\n39\n40\n41\n42\n43\n44\n45\n46\n47\n48\n49\n50\n"
"#
        );

        assert_eq!(
            but_testsupport::visualize_tree(
                outcome
                    .changed_tree_pre_cherry_pick
                    .expect("present if new commit is present")
                    .attach(&repo),
            )
            .to_string(),
            r#"2f19efb
└── file:100644:33e9beb "20\n21\n22\n23\n24\n25\n26\n27\n28\n29\n30\n31\n32\n33\n34\n35\n36\n37\n38\n39\n40\n1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n11\n12\n13\n14\n15\n16\n17\n18\n19\n20\n21\n22\n23\n24\n25\n26\n27\n28\n29\n30\n31\n32\n33\n34\n35\n36\n37\n38\n39\n40\n41\n42\n43\n44\n45\n46\n47\n48\n49\n50\n51\n52\n53\n54\n55\n56\n57\n58\n59\n60\n61\n62\n63\n64\n65\n66\n67\n68\n69\n70\n71\n72\n73\n74\n75\n76\n77\n78\n79\n80\n30\n31\n32\n33\n34\n35\n36\n37\n38\n39\n40\n41\n42\n43\n44\n45\n46\n47\n48\n49\n50\n"
"#
        );
    }
    Ok(())
}

#[test]
fn commit_to_branches_below_merge_commit() -> anyhow::Result<()> {
    assure_stable_env();

    let (repo, _tmp) = writable_scenario("merge-with-two-branches-line-offset");

    write_sequence(&repo, "file", [(1, 20), (40, 50)])?;
    let outcome = commit_whole_files_and_all_hunks_from_workspace(
        &repo,
        Destination::NewCommit {
            parent_commit_id: Some(repo.rev_parse_single("B")?.into()),
            message: "a new commit onto B, changing only the lines that it wrote".into(),
            stack_segment: None,
        },
    )?;

    let tree = visualize_tree(&repo, &outcome)?;
    insta::assert_snapshot!(tree, @r#"
    a38c1c3
    └── file:100644:12121fe "10\n11\n12\n13\n14\n15\n16\n17\n18\n19\n20\n40\n41\n42\n43\n44\n45\n46\n47\n48\n49\n50\n"
    "#);

    write_sequence(&repo, "file", [(40, 50), (10, 30)])?;
    let outcome = commit_whole_files_and_all_hunks_from_workspace(
        &repo,
        Destination::NewCommit {
            parent_commit_id: Some(repo.rev_parse_single("A")?.into()),
            message: "a new commit onto A, changing only the lines that it wrote".into(),
            stack_segment: None,
        },
    )?;

    let tree = visualize_tree(&repo, &outcome)?;
    insta::assert_snapshot!(tree, @r#"
    704f5ca
    └── file:100644:bc33e02 "40\n41\n42\n43\n44\n45\n46\n47\n48\n49\n50\n10\n11\n12\n13\n14\n15\n16\n17\n18\n19\n20\n"
    "#);

    insta::assert_snapshot!(but_testsupport::visualize_tree(outcome.changed_tree_pre_cherry_pick.unwrap().attach(&repo)), @r#"
    3cca5b3
    └── file:100644:144ccb0 "40\n41\n42\n43\n44\n45\n46\n47\n48\n49\n50\n10\n11\n12\n13\n14\n15\n16\n17\n18\n19\n20\n21\n22\n23\n24\n25\n26\n27\n28\n29\n30\n"
    "#);
    Ok(())
}

#[test]
fn commit_whole_file_to_conflicting_position() -> anyhow::Result<()> {
    assure_stable_env();

    let (repo, _tmp) = writable_scenario("merge-with-two-branches-line-offset");

    // rewrite all lines so changes cover both branches
    write_sequence(&repo, "file", [(40, 70)])?;
    for conflicting_parent_commit in ["A", "B", "main"] {
        let parent_commit = repo.rev_parse_single(conflicting_parent_commit)?;
        let outcome = commit_whole_files_and_all_hunks_from_workspace(
            &repo,
            Destination::NewCommit {
                parent_commit_id: Some(parent_commit.into()),
                message: "this commit can't be done as it covers multiple commits, \
            which will conflict on cherry-picking"
                    .into(),
                stack_segment: None,
            },
        )?;

        assert_ne!(
            outcome.new_commit, None,
            "Everything fails, so we create a commit anyway (despite no change happened) to help the user deal with it"
        );
        // The hunks are never present, as they always match, further clarifying that the hunks aren't the problem.
        insta::allow_duplicates! {
        insta::assert_debug_snapshot!(outcome.rejected_specs, @r#"
        [
            (
                CherryPickMergeConflict,
                DiffSpec {
                    previous_path: None,
                    path: "file",
                    hunk_headers: [],
                },
            ),
        ]
        "#)}
    }

    let outcome = commit_whole_files_and_all_hunks_from_workspace(
        &repo,
        Destination::NewCommit {
            parent_commit_id: Some(repo.head_id()?.into()),
            message: "but it can be applied directly to the tip, the merge commit itself, it always works".into(),
            stack_segment: None,
        },
    )?;
    let tree = visualize_tree(&repo, &outcome)?;
    insta::assert_snapshot!(tree, @r#"
    5bbee6d
    └── file:100644:1c9325b "40\n41\n42\n43\n44\n45\n46\n47\n48\n49\n50\n51\n52\n53\n54\n55\n56\n57\n58\n59\n60\n61\n62\n63\n64\n65\n66\n67\n68\n69\n70\n"
    "#);
    Ok(())
}

#[test]
fn commit_whole_file_to_conflicting_position_one_unconflicting_file_remains() -> anyhow::Result<()>
{
    assure_stable_env();

    let (repo, _tmp) = writable_scenario("merge-with-two-branches-line-offset-two-files");

    // rewrite all lines so changes cover both branches
    write_sequence(&repo, "file", [(40, 70)])?;
    // Change the second file to be non-conflicting, just the half the lines in the middle
    write_sequence(&repo, "other-file", [(35, 44), (80, 90), (66, 75)])?;
    for conflicting_parent_commit in ["A", "B", "main"] {
        let parent_commit = repo.rev_parse_single(conflicting_parent_commit)?;
        let outcome = commit_whole_files_and_all_hunks_from_workspace(
            &repo,
            Destination::NewCommit {
                parent_commit_id: Some(parent_commit.into()),
                message: "this commit can't be done as it covers multiple commits, \
            which will conflict on cherry-picking"
                    .into(),
                stack_segment: None,
            },
        )?;
        assert_ne!(
            outcome.new_commit, None,
            "Not everything fails, so there is a commit"
        );
        // The hunks are never present, as they always match, further clarifying that the hunks aren't the problem.
        insta::allow_duplicates! {
        insta::assert_debug_snapshot!(outcome.rejected_specs, @r#"
        [
            (
                CherryPickMergeConflict,
                DiffSpec {
                    previous_path: None,
                    path: "file",
                    hunk_headers: [],
                },
            ),
        ]
        "#)}
        // Different bases mean different base versions for the conflicting file.
        if conflicting_parent_commit == "A" {
            insta::assert_snapshot!(visualize_tree(&repo, &outcome)?, @r#"
            0816d13
            ├── file:100644:0ff3bbb "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n11\n12\n13\n14\n15\n16\n17\n18\n19\n20\n"
            └── other-file:100644:593469b "35\n36\n37\n38\n39\n40\n41\n42\n43\n44\n80\n81\n82\n83\n84\n85\n86\n87\n88\n89\n90\n"
            "#);
        } else if conflicting_parent_commit == "B" {
            insta::assert_snapshot!(visualize_tree(&repo, &outcome)?, @r#"
            df6d629
            ├── file:100644:1f1542b "10\n11\n12\n13\n14\n15\n16\n17\n18\n19\n20\n21\n22\n23\n24\n25\n26\n27\n28\n29\n30\n"
            └── other-file:100644:a935ec9 "80\n81\n82\n83\n84\n85\n86\n87\n88\n89\n90\n66\n67\n68\n69\n70\n71\n72\n73\n74\n75\n"
            "#);
        } else if conflicting_parent_commit == "main" {
            insta::assert_snapshot!(visualize_tree(&repo, &outcome)?, @r#"
            d5d6e30
            ├── file:100644:e33f5e9 "10\n11\n12\n13\n14\n15\n16\n17\n18\n19\n20\n"
            └── other-file:100644:240fe08 "80\n81\n82\n83\n84\n85\n86\n87\n88\n89\n90\n"
            "#);
        }
    }

    let outcome = commit_whole_files_and_all_hunks_from_workspace(
        &repo,
        Destination::NewCommit {
            parent_commit_id: Some(repo.head_id()?.into()),
            message: "but it can be applied directly to the tip, \
            the merge commit itself, it always works"
                .into(),
            stack_segment: None,
        },
    )?;
    let tree = visualize_tree(&repo, &outcome)?;
    insta::assert_snapshot!(tree, @r#"
    7d017dd
    ├── file:100644:1c9325b "40\n41\n42\n43\n44\n45\n46\n47\n48\n49\n50\n51\n52\n53\n54\n55\n56\n57\n58\n59\n60\n61\n62\n63\n64\n65\n66\n67\n68\n69\n70\n"
    └── other-file:100644:4223e57 "35\n36\n37\n38\n39\n40\n41\n42\n43\n44\n80\n81\n82\n83\n84\n85\n86\n87\n88\n89\n90\n66\n67\n68\n69\n70\n71\n72\n73\n74\n75\n"
    "#);
    Ok(())
}

#[test]
fn unborn_untracked_worktree_filters_are_applied_to_whole_files() -> anyhow::Result<()> {
    assure_stable_env();

    let (repo, _tmp) = writable_scenario("unborn-untracked-crlf");
    let outcome = commit_whole_files_and_all_hunks_from_workspace(
        &repo,
        Destination::NewCommit {
            parent_commit_id: None,
            message: "the commit message".into(),
            stack_segment: None,
        },
    )?;
    insta::assert_debug_snapshot!(&outcome, @r"
    CreateCommitOutcome {
        rejected_specs: [],
        new_commit: Some(
            Sha1(81dee909affdf17107ffdee354d682fb36c82f78),
        ),
        changed_tree_pre_cherry_pick: Some(
            Sha1(d5949f12727c8e89e1351b89e8e510dfa1e2adc9),
        ),
        references: [],
        rebase_output: None,
        index: None,
    }
    ");

    let new_commit_id = outcome.new_commit.expect("a new commit was created");
    let new_commit = new_commit_id.attach(&repo).object()?.peel_to_commit()?;
    assert_eq!(new_commit.message_raw()?, "the commit message");

    // What's in Git is unix style newlines
    let tree = but_testsupport::visualize_tree(new_commit.tree_id()?);
    insta::assert_snapshot!(tree, @r#"
    d5949f1
    └── not-yet-tracked:100644:1191247 "1\n2\n"
    "#);

    std::fs::write(
        repo.workdir_path("new-untracked").expect("non-bare"),
        "one\r\ntwo\r\n",
    )?;
    let outcome = commit_whole_files_and_all_hunks_from_workspace(
        &repo,
        Destination::NewCommit {
            parent_commit_id: Some(new_commit_id),
            message: "the second commit".into(),
            stack_segment: None,
        },
    )?;

    insta::assert_debug_snapshot!(&outcome, @r"
    CreateCommitOutcome {
        rejected_specs: [],
        new_commit: Some(
            Sha1(d736d5adfcad413d89d99c0f30f03a0dde606ab1),
        ),
        changed_tree_pre_cherry_pick: Some(
            Sha1(cef74127e0e9f4c46b5ff360d6208ee0cc839eba),
        ),
        references: [],
        rebase_output: None,
        index: None,
    }
    ");

    let tree = visualize_tree(&repo, &outcome)?;
    insta::assert_snapshot!(tree, @r#"
    cef7412
    ├── new-untracked:100644:814f4a4 "one\ntwo\n"
    └── not-yet-tracked:100644:1191247 "1\n2\n"
    "#);

    Ok(())
}

#[test]
fn signatures_are_redone() -> anyhow::Result<()> {
    assure_stable_env();

    let (repo, _tmp) = writable_scenario_with_ssh_key("two-signed-commits-with-line-offset");

    let head_id = repo.head_id()?;
    let head_commit = head_id.object()?.into_commit().decode()?.to_owned();
    let head_id = head_id.detach();
    let previous_signature = head_commit
        .extra_headers()
        .pgp_signature()
        .expect("it's signed by default");

    // Rewrite everything for amending on top.
    write_sequence(&repo, "file", [(40, 60)])?;
    let outcome = commit_whole_files_and_all_hunks_from_workspace(
        &repo,
        Destination::NewCommit {
            parent_commit_id: Some(head_id),
            message: "a commit with signature".into(),
            stack_segment: None,
        },
    )?;

    let new_commit = commit_from_outcome(&repo, &outcome)?;
    let new_signature = new_commit
        .extra_headers()
        .pgp_signature()
        .expect("signing config is respected");
    assert_ne!(
        previous_signature, new_signature,
        "signatures are recreated as the commit is changed"
    );
    assert_eq!(
        new_commit
            .extra_headers()
            .find_all(gix::objs::commit::SIGNATURE_FIELD_NAME)
            .count(),
        1,
        "it doesn't leave outdated signatures on top of the updated one"
    );
    insta::assert_snapshot!(visualize_tree(&repo, &outcome)?, @r#"
    3412b2c
    ├── .gitignore:100644:ccc87a0 "*.key*\n"
    └── file:100644:a07b65a "40\n41\n42\n43\n44\n45\n46\n47\n48\n49\n50\n51\n52\n53\n54\n55\n56\n57\n58\n59\n60\n"
    "#);

    Ok(())
}

#[test]
fn validate_no_change_on_noop() -> anyhow::Result<()> {
    assure_stable_env();

    let repo = read_only_in_memory_scenario("two-commits-with-line-offset")?;
    let specs = vec![DiffSpec {
        path: "file".into(),
        ..Default::default()
    }];
    let outcome = commit_engine::create_commit(
        &repo,
        Destination::NewCommit {
            parent_commit_id: Some(repo.head_id()?.into()),
            message: "the file has no worktree changes even though we claim it - \
        so it's rejected and no new commit is created"
                .into(),
            stack_segment: None,
        },
        None,
        specs.clone(),
        CONTEXT_LINES,
    )?;
    assert_eq!(
        outcome.new_commit, None,
        "no new commit is returned as no change actually happened"
    );
    insta::assert_debug_snapshot!(&outcome, @r#"
    CreateCommitOutcome {
        rejected_specs: [
            (
                NoEffectiveChanges,
                DiffSpec {
                    previous_path: None,
                    path: "file",
                    hunk_headers: [],
                },
            ),
        ],
        new_commit: None,
        changed_tree_pre_cherry_pick: None,
        references: [],
        rebase_output: None,
        index: None,
    }
    "#);
    Ok(())
}

const UI_CONTEXT_LINES: u32 = 3;

mod utils {
    use bstr::BString;
    use but_core::UnifiedPatch;

    pub fn worktree_change_diffs(
        repo: &gix::Repository,
        context_lines: u32,
    ) -> anyhow::Result<Vec<(Option<BString>, BString, UnifiedPatch)>> {
        Ok(but_core::diff::worktree_changes(repo)?
            .changes
            .iter()
            .map(|c| {
                (
                    c.previous_path().map(ToOwned::to_owned),
                    c.path.clone(),
                    c.unified_patch(repo, context_lines).unwrap().unwrap(),
                )
            })
            .collect())
    }
}
