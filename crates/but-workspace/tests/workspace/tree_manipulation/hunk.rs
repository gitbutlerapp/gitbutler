use bstr::{BString, ByteSlice};
use but_core::{DiffSpec, HunkHeader, UnifiedPatch};
use but_testsupport::{git_status, hunk_header, visualize_disk_tree_skip_dot_git};
use but_workspace::discard_workspace_changes;

use crate::{
    tree_manipulation::hunk::util::{changed_file_in_worktree_with_hunks, previous_change_text},
    utils::{
        CONTEXT_LINES, read_only_in_memory_scenario, to_change_specs_all_hunks, visualize_index,
        writable_scenario,
    },
};

#[test]
fn dropped_hunks() -> anyhow::Result<()> {
    let (repo, _tmp) = writable_scenario("mixed-hunk-modifications");
    let (change, hunks) = changed_file_in_worktree_with_hunks(&repo, "file", CONTEXT_LINES)?;

    let mut hunks_to_discard: Vec<HunkHeader> = hunks.into_iter().map(Into::into).collect();
    hunks_to_discard.push(hunk_header("-10,1", "+13,3"));
    hunks_to_discard.insert(0, hunk_header("-1,1", "+1,0"));

    let discard_spec = DiffSpec {
        previous_path: None,
        path: change.path,
        hunk_headers: hunks_to_discard,
    };
    let dropped = discard_workspace_changes(&repo, Some(discard_spec), CONTEXT_LINES)?;
    // It drops just the two missing ones hunks
    insta::assert_debug_snapshot!(dropped, @r#"
    [
        DiffSpec {
            previous_path: None,
            path: "file",
            hunk_headers: [
                HunkHeader("-1,1", "+1,0"),
                HunkHeader("-10,1", "+13,3"),
            ],
        },
    ]
    "#);
    Ok(())
}

#[test]
fn non_modifications_trigger_error() -> anyhow::Result<()> {
    let repo = read_only_in_memory_scenario("deletion-addition-untracked")?;
    insta::assert_snapshot!(git_status(&repo)?, @r"
    A  added-to-index
     D to-be-deleted
    D  to-be-deleted-in-index
    ?? untracked
    ");

    let add_single_line = hunk_header("-1,0", "+1,1");
    let remove_single_line = hunk_header("-1,1", "+1,0");
    for (file_name, hunk) in [
        ("untracked", add_single_line),
        ("added-to-index", add_single_line),
        ("to-be-deleted", remove_single_line),
        ("to-be-deleted-in-index", remove_single_line),
    ] {
        let err = discard_workspace_changes(
            &repo,
            Some(DiffSpec {
                previous_path: None,
                path: file_name.into(),
                hunk_headers: vec![hunk],
            }),
            CONTEXT_LINES,
        )
        .unwrap_err();
        assert!(err.to_string().starts_with(
            "Deletions or additions aren't well-defined for hunk-based operations - use the whole-file mode instead"
        ),);
    }
    Ok(())
}

#[test]
fn from_end() -> anyhow::Result<()> {
    let (repo, _tmp) = writable_scenario("mixed-hunk-modifications");
    let mut hunk_info = Vec::new();
    let filename = "file-in-index";
    let file_content = || std::fs::read(repo.workdir().unwrap().join(filename)).map(BString::from);
    insta::assert_snapshot!(file_content()?, @r"
    1
    2
    3
    4
    5
    6-7
    8
    9
    ten
    eleven
    12
    20
    21
    22
    15
    16
    ");
    while let Some(change) = but_core::diff::worktree_changes(&repo)?
        .changes
        .into_iter()
        .find(|change| change.path == filename)
    {
        let previous_text = previous_change_text(&repo, &change)?;
        insta::allow_duplicates!(insta::assert_snapshot!(previous_text, @r"
        5
        6
        7
        8
        9
        10
        11
        12
        13
        14
        15
        16
        17
        18
        "));
        let Some(UnifiedPatch::Patch { mut hunks, .. }) =
            change.unified_patch(&repo, CONTEXT_LINES)?
        else {
            unreachable!("We know there are hunks")
        };
        assert_ne!(
            hunks.len(),
            0,
            "the reason we see it is file modifications: {change:#?}"
        );

        let before = file_content()?;
        let mut last_hunk = hunks
            .pop()
            .expect("there is always one change if the file is only modified");
        let discarded_patch = std::mem::take(&mut last_hunk.diff);
        let discard_spec = DiffSpec {
            previous_path: None,
            path: change.path.clone(),
            hunk_headers: vec![last_hunk.into()],
        };
        let dropped = discard_workspace_changes(&repo, Some(discard_spec), CONTEXT_LINES)?;
        assert_eq!(
            dropped.len(),
            0,
            "the hunk could be found and was discarded"
        );
        let after = file_content()?;
        hunk_info.push((before, discarded_patch, after));
    }

    insta::assert_debug_snapshot!(hunk_info, @r#"
    [
        (
            "1\n2\n3\n4\n5\n6-7\n8\n9\nten\neleven\n12\n20\n21\n22\n15\n16\n",
            "@@ -13,2 +17,0 @@\n-17\n-18\n",
            "1\n2\n3\n4\n5\n6-7\n8\n9\nten\neleven\n12\n20\n21\n22\n15\n16\n17\n18\n",
        ),
        (
            "1\n2\n3\n4\n5\n6-7\n8\n9\nten\neleven\n12\n20\n21\n22\n15\n16\n17\n18\n",
            "@@ -9,2 +12,3 @@\n-13\n-14\n+20\n+21\n+22\n",
            "1\n2\n3\n4\n5\n6-7\n8\n9\nten\neleven\n12\n13\n14\n15\n16\n17\n18\n",
        ),
        (
            "1\n2\n3\n4\n5\n6-7\n8\n9\nten\neleven\n12\n13\n14\n15\n16\n17\n18\n",
            "@@ -6,2 +9,2 @@\n-10\n-11\n+ten\n+eleven\n",
            "1\n2\n3\n4\n5\n6-7\n8\n9\n10\n11\n12\n13\n14\n15\n16\n17\n18\n",
        ),
        (
            "1\n2\n3\n4\n5\n6-7\n8\n9\n10\n11\n12\n13\n14\n15\n16\n17\n18\n",
            "@@ -2,2 +6,1 @@\n-6\n-7\n+6-7\n",
            "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n11\n12\n13\n14\n15\n16\n17\n18\n",
        ),
        (
            "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n11\n12\n13\n14\n15\n16\n17\n18\n",
            "@@ -1,0 +1,4 @@\n+1\n+2\n+3\n+4\n",
            "5\n6\n7\n8\n9\n10\n11\n12\n13\n14\n15\n16\n17\n18\n",
        ),
    ]
    "#);
    Ok(())
}

#[test]
fn from_beginning() -> anyhow::Result<()> {
    let (repo, _tmp) = writable_scenario("mixed-hunk-modifications");
    let mut hunk_info = Vec::new();
    let filename = "file-in-index";
    let file_content = || std::fs::read(repo.workdir().unwrap().join(filename)).map(BString::from);
    while let Some(change) = but_core::diff::worktree_changes(&repo)?
        .changes
        .into_iter()
        .find(|change| change.path == filename)
    {
        let Some(UnifiedPatch::Patch { mut hunks, .. }) =
            change.unified_patch(&repo, CONTEXT_LINES)?
        else {
            unreachable!("We know there are hunks")
        };
        assert_ne!(
            hunks.len(),
            0,
            "the reason we see it is file modifications: {change:#?}"
        );

        let before = file_content()?;
        let mut first_hun_hunk = hunks.remove(0);
        let discarded_patch = std::mem::take(&mut first_hun_hunk.diff);
        let discard_spec = DiffSpec {
            previous_path: None,
            path: change.path.clone(),
            hunk_headers: vec![first_hun_hunk.into()],
        };
        let dropped = discard_workspace_changes(&repo, Some(discard_spec), CONTEXT_LINES)?;
        assert_eq!(
            dropped.len(),
            0,
            "the hunk could be found and was discarded"
        );
        let after = file_content()?;
        hunk_info.push((before, discarded_patch, after));
    }

    insta::assert_debug_snapshot!(hunk_info, @r#"
    [
        (
            "1\n2\n3\n4\n5\n6-7\n8\n9\nten\neleven\n12\n20\n21\n22\n15\n16\n",
            "@@ -1,0 +1,4 @@\n+1\n+2\n+3\n+4\n",
            "5\n6-7\n8\n9\nten\neleven\n12\n20\n21\n22\n15\n16\n",
        ),
        (
            "5\n6-7\n8\n9\nten\neleven\n12\n20\n21\n22\n15\n16\n",
            "@@ -2,2 +2,1 @@\n-6\n-7\n+6-7\n",
            "5\n6\n7\n8\n9\nten\neleven\n12\n20\n21\n22\n15\n16\n",
        ),
        (
            "5\n6\n7\n8\n9\nten\neleven\n12\n20\n21\n22\n15\n16\n",
            "@@ -6,2 +6,2 @@\n-10\n-11\n+ten\n+eleven\n",
            "5\n6\n7\n8\n9\n10\n11\n12\n20\n21\n22\n15\n16\n",
        ),
        (
            "5\n6\n7\n8\n9\n10\n11\n12\n20\n21\n22\n15\n16\n",
            "@@ -9,2 +9,3 @@\n-13\n-14\n+20\n+21\n+22\n",
            "5\n6\n7\n8\n9\n10\n11\n12\n13\n14\n15\n16\n",
        ),
        (
            "5\n6\n7\n8\n9\n10\n11\n12\n13\n14\n15\n16\n",
            "@@ -13,2 +13,0 @@\n-17\n-18\n",
            "5\n6\n7\n8\n9\n10\n11\n12\n13\n14\n15\n16\n17\n18\n",
        ),
    ]
    "#);
    Ok(())
}

#[test]
fn from_selections() -> anyhow::Result<()> {
    let (repo, _tmp) = writable_scenario("mixed-hunk-modifications");
    let filename = "file-in-index";
    let (change, hunks) = changed_file_in_worktree_with_hunks(&repo, filename, CONTEXT_LINES)?;
    insta::assert_debug_snapshot!(hunks.iter().map(|h| &h.diff).collect::<Vec<_>>(), @r#"
    [
        "@@ -1,0 +1,4 @@\n+1\n+2\n+3\n+4\n",
        "@@ -2,2 +6,1 @@\n-6\n-7\n+6-7\n",
        "@@ -6,2 +9,2 @@\n-10\n-11\n+ten\n+eleven\n",
        "@@ -9,2 +12,3 @@\n-13\n-14\n+20\n+21\n+22\n",
        "@@ -13,2 +17,0 @@\n-17\n-18\n",
    ]
    "#);

    let discard_spec = DiffSpec {
        previous_path: None,
        path: change.path.clone(),
        hunk_headers: vec![
            // Split first hunk into two yielding
            // '+1\n+3\n+4\n'
            hunk_header("-1,0", "+2,1"),
            // A complete header can be mixed in as well to undo the whole change.
            hunk_header("-2,2", "+6,1"),
            // Discard '-10\n-11\n+ten\n+eleven\n' by discarding both lines individually.
            hunk_header("-6,2", "+9,1"),
            hunk_header("-6,2", "+10,1"),
            // Discard the beginning and the end to yield
            // '+21\n'
            hunk_header("-9,2", "+12,1"),
            hunk_header("-9,2", "+14,1"),
            // Discard only the last line to yield.
            // '-18\n'
            hunk_header("-14,1", "+17,0"),
        ],
    };
    let dropped = discard_workspace_changes(&repo, Some(discard_spec), CONTEXT_LINES)?;
    assert_eq!(dropped, [], "all sub-hunks could be associated");

    let file_content: BString = std::fs::read(repo.workdir().unwrap().join(filename))?.into();
    insta::assert_snapshot!(file_content, @r"
    1
    3
    4
    5
    6
    7
    8
    9
    12
    21
    15
    16
    18
    ");

    Ok(())
}

#[test]
fn from_selections_with_context() -> anyhow::Result<()> {
    let (repo, _tmp) = writable_scenario("mixed-hunk-modifications");
    let filename = "file-in-index";
    let ui_context_lines = 3;
    let (change, hunks) = changed_file_in_worktree_with_hunks(&repo, filename, ui_context_lines)?;
    assert_eq!(
        hunks.len(),
        1,
        "one big hunk with context and everything, similar to what the UI sees"
    );
    insta::assert_snapshot!(hunks[0].diff, @r"
    @@ -1,14 +1,16 @@
    +1
    +2
    +3
    +4
     5
    -6
    -7
    +6-7
     8
     9
    -10
    -11
    +ten
    +eleven
     12
    -13
    -14
    +20
    +21
    +22
     15
     16
    -17
    -18
    ");

    let filepath = repo.workdir().unwrap().join(filename);
    let read_file_content = || std::fs::read(&filepath).map(BString::from);
    let original_file_content = read_file_content()?;
    let mut discard_spec = DiffSpec {
        previous_path: None,
        path: change.path.clone(),
        hunk_headers: vec![
            // Discard 2,3, keeping 1,4
            hunk_header("-1,14", "+2,2"),
            // Get 6,7 back
            hunk_header("-2,2", "+1,16"),
            // Remove 6-7
            hunk_header("-1,14", "+6,1"),
            // Get 11 back
            hunk_header("-7,1", "+1,16"),
            // Remove 'ten'
            hunk_header("-1,14", "+9,1"),
            // Remove 20,21,22
            hunk_header("-1,14", "+12,3"),
            // Get 17,18 back
            hunk_header("-13,2", "+1,16"),
        ],
    };
    let dropped = discard_workspace_changes(&repo, Some(discard_spec.clone()), ui_context_lines)?;
    assert_eq!(dropped.len(), 0, "all sub-hunks could be associated");

    let file_content = read_file_content()?;
    insta::assert_snapshot!(file_content, @r"
    1
    6
    7
    4
    5
    8
    11
    9
    eleven
    12
    15
    16
    17
    18
    ");

    std::fs::write(&filepath, original_file_content)?;
    discard_spec.hunk_headers.reverse();
    let dropped = discard_workspace_changes(&repo, Some(discard_spec.clone()), ui_context_lines)?;
    assert_eq!(
        dropped.len(),
        0,
        "hunk-selection order doesn't matter, they can still be associated"
    );
    let actual = read_file_content()?;
    // The order of old/new additions or removals do matter also doesn't matter, the result is stable.
    insta::assert_snapshot!(actual, @r"
    1
    6
    7
    4
    5
    8
    11
    9
    eleven
    12
    15
    16
    17
    18
    ");

    Ok(())
}

#[test]
fn hunk_removal_of_additions_single_line() -> anyhow::Result<()> {
    let (repo, _tmp) = writable_scenario("plain-modifications");
    let filename = "all-added";
    let (change, hunks) = changed_file_in_worktree_with_hunks(&repo, filename, CONTEXT_LINES)?;
    assert_eq!(
        hunks.len(),
        1,
        "one big hunk with context and everything, similar to what the UI sees"
    );
    insta::assert_snapshot!(hunks[0].diff, @r"
    @@ -1,0 +1,10 @@
    +1
    +2
    +3
    +4
    +5
    +6
    +7
    +8
    +9
    +10
    ");

    let discard_spec = DiffSpec {
        previous_path: None,
        path: change.path.clone(),
        hunk_headers: vec![
            // Anchor at the old hunk, and redefine change to discard in the new hunk,
            // effectively discarding only line 5.
            // Internally we turn this into [("-1,0", "+1,4"), ("-1,0", "+6,4")].
            hunk_header("-1,0", "+5,1"),
            // TODO: figure out a header specification
        ],
    };
    let dropped = discard_workspace_changes(&repo, Some(discard_spec), CONTEXT_LINES)?;
    assert_eq!(dropped.len(), 0, "all sub-hunks could be associated");

    let file_content: BString = std::fs::read(repo.workdir().unwrap().join(filename))?.into();
    insta::assert_snapshot!(file_content, @r"
    1
    2
    3
    4
    6
    7
    8
    9
    10
    ");

    Ok(())
}

#[test]
fn hunk_removal_of_removal_single_line() -> anyhow::Result<()> {
    let (repo, _tmp) = writable_scenario("plain-modifications");
    let filename = "all-removed";
    let (change, hunks) = changed_file_in_worktree_with_hunks(&repo, filename, CONTEXT_LINES)?;
    assert_eq!(
        hunks.len(),
        1,
        "one big hunk with context and everything, similar to what the UI sees"
    );
    insta::assert_snapshot!(hunks[0].diff, @r"
    @@ -1,10 +1,0 @@
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
    ");

    let discard_spec = DiffSpec {
        previous_path: None,
        path: change.path.clone(),
        hunk_headers: vec![
            // Anchor at the new hunk, and redefine change to discard in the old hunk,
            // effectively keeping only line 5.
            // Internally we turn this into [("-1,5", "+1,0"), ("-6,4", "+1,0")].
            hunk_header("-5,1", "+1,0"),
        ],
    };
    let dropped = discard_workspace_changes(&repo, Some(discard_spec), CONTEXT_LINES)?;
    assert_eq!(dropped.len(), 0, "all sub-hunks could be associated");

    let file_content: BString = std::fs::read(repo.workdir().unwrap().join(filename))?.into();
    insta::assert_snapshot!(file_content, @"5");

    Ok(())
}

#[test]
fn hunk_removal_of_modifications() -> anyhow::Result<()> {
    let (repo, _tmp) = writable_scenario("plain-modifications");
    let filename = "all-modified";
    let (change, hunks) = changed_file_in_worktree_with_hunks(&repo, filename, CONTEXT_LINES)?;
    assert_eq!(
        hunks.len(),
        1,
        "one big hunk with context and everything, similar to what the UI sees"
    );
    insta::assert_snapshot!(hunks[0].diff, @r"
    @@ -1,10 +1,10 @@
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
    ");

    let discard_spec = DiffSpec {
        previous_path: None,
        path: change.path.clone(),
        hunk_headers: vec![
            // Anchor at the new hunk, and redefine change to discard in the old hunk,
            // effectively keeping only line 5.
            // Internally we turn this into '[("-1,4", "+1,4"), ("-6,5", "+5,6")]', which deletes the surrounding
            // of line 5 in the old patch, essentially surrounding it with the new image which looses no line.
            hunk_header("-5,1", "+1,10"),
            // Anchor undoing the addition to the old patch (with old offsets) and specify the selection to discard.
            // This will yield '[("-1,4", "+1,4"), ("-6,5", "+6,5")]' internally.
            hunk_header("-1,10", "+5,1"),
        ],
    };

    let dropped = discard_workspace_changes(&repo, Some(discard_spec), CONTEXT_LINES)?;
    assert_eq!(dropped.len(), 0, "all sub-hunks could be associated");

    let file_content: BString = std::fs::read(repo.workdir().unwrap().join(filename))?.into();
    insta::assert_snapshot!(file_content, @r"
    11
    12
    13
    14
    5
    16
    17
    18
    19
    20
    ");

    Ok(())
}

#[test]
#[cfg(unix)]
fn deletion_modification_addition_of_hunks_mixed_discard_all_in_workspace() -> anyhow::Result<()> {
    let (repo, _tmp) = writable_scenario("mixed-hunk-modifications");
    // Note that one of these renames can't be detected by Git but is visible to us.
    insta::assert_snapshot!(git_status(&repo)?, @r"
     M file
    M  file-in-index
    RM file-to-be-renamed-in-index -> file-renamed-in-index
     D file-to-be-renamed
    ?? file-renamed
    ");

    insta::assert_snapshot!(visualize_index(&**repo.index()?), @r"
    100755:3d3b36f file
    100755:cb89473 file-in-index
    100644:3d3b36f file-renamed-in-index
    100644:3d3b36f file-to-be-renamed
    ");

    let workdir = repo.workdir().unwrap();
    insta::assert_snapshot!(visualize_disk_tree_skip_dot_git(workdir)?, @r"
    .
    ├── .git:40755
    ├── file:100644
    ├── file-in-index:100755
    ├── file-renamed:100755
    └── file-renamed-in-index:100644
    ");

    // Show that we detect renames correctly, despite the rename + modification.
    let wt_changes = but_core::diff::worktree_changes(&repo)?;
    insta::assert_debug_snapshot!(wt_changes, @r#"
    WorktreeChanges {
        changes: [
            TreeChange {
                path: "file",
                status: Modification {
                    previous_state: ChangeState {
                        id: Sha1(3d3b36f021391fa57312d7dfd1ad8cf5a13dca6d),
                        kind: BlobExecutable,
                    },
                    state: ChangeState {
                        id: Sha1(0000000000000000000000000000000000000000),
                        kind: Blob,
                    },
                    flags: Some(
                        ExecutableBitRemoved,
                    ),
                },
            },
            TreeChange {
                path: "file-in-index",
                status: Modification {
                    previous_state: ChangeState {
                        id: Sha1(3d3b36f021391fa57312d7dfd1ad8cf5a13dca6d),
                        kind: Blob,
                    },
                    state: ChangeState {
                        id: Sha1(cb89473a55c3443b5567e990e2a0293895c91a4a),
                        kind: BlobExecutable,
                    },
                    flags: Some(
                        ExecutableBitAdded,
                    ),
                },
            },
            TreeChange {
                path: "file-renamed",
                status: Rename {
                    previous_path: "file-to-be-renamed",
                    previous_state: ChangeState {
                        id: Sha1(3d3b36f021391fa57312d7dfd1ad8cf5a13dca6d),
                        kind: Blob,
                    },
                    state: ChangeState {
                        id: Sha1(0000000000000000000000000000000000000000),
                        kind: BlobExecutable,
                    },
                    flags: Some(
                        ExecutableBitAdded,
                    ),
                },
            },
            TreeChange {
                path: "file-renamed-in-index",
                status: Rename {
                    previous_path: "file-to-be-renamed-in-index",
                    previous_state: ChangeState {
                        id: Sha1(3d3b36f021391fa57312d7dfd1ad8cf5a13dca6d),
                        kind: Blob,
                    },
                    state: ChangeState {
                        id: Sha1(0000000000000000000000000000000000000000),
                        kind: Blob,
                    },
                    flags: None,
                },
            },
        ],
        ignored_changes: [
            IgnoredWorktreeChange {
                path: "file-renamed-in-index",
                status: TreeIndex,
            },
        ],
    }
    "#);

    let specs = to_change_specs_all_hunks(&repo, wt_changes)?;
    let dropped = discard_workspace_changes(&repo, specs.into_iter(), CONTEXT_LINES)?;
    assert!(dropped.is_empty());

    // Only the data is undone; the executable bit change can be undone in the next discard,
    // making this a two-step process. This seems valuable as it gives users a choice.
    insta::assert_snapshot!(visualize_disk_tree_skip_dot_git(repo.workdir().unwrap())?, @r"
    .
    ├── .git:40755
    ├── file:100644
    ├── file-in-index:100755
    ├── file-renamed:100755
    └── file-renamed-in-index:100644
    ");

    for filename in [
        "file",
        "file-in-index",
        "file-renamed",
        "file-renamed-in-index",
    ] {
        let content = std::fs::read(workdir.join(filename))?;
        assert_eq!(
            content.as_bstr(),
            "5\n6\n7\n8\n9\n10\n11\n12\n13\n14\n15\n16\n17\n18\n",
            "{filename}: All files have the same content after worktree-discards"
        );
    }

    // Notably, discarding all hunks leaves the renamed file in place, but without modifications.
    // Executable bits stay and can be discarded in a separate step.
    insta::assert_snapshot!(git_status(&repo)?, @r"
     M file
    MM file-in-index
    R  file-to-be-renamed-in-index -> file-renamed-in-index
     D file-to-be-renamed
    ?? file-renamed
    ");
    // The index still only holds what was in the index before, but is representing the changed worktree.
    insta::assert_snapshot!(visualize_index(&**repo.index()?), @r"
    100755:3d3b36f file
    100755:cb89473 file-in-index
    100644:3d3b36f file-renamed-in-index
    100644:3d3b36f file-to-be-renamed
    ");

    // The index is transparent, so `file-in-index` was reverted to the version in the `HEAD^{tree}`
    let wt_changes = but_core::diff::worktree_changes(&repo)?;
    insta::assert_debug_snapshot!(wt_changes, @r#"
    WorktreeChanges {
        changes: [
            TreeChange {
                path: "file",
                status: Modification {
                    previous_state: ChangeState {
                        id: Sha1(3d3b36f021391fa57312d7dfd1ad8cf5a13dca6d),
                        kind: BlobExecutable,
                    },
                    state: ChangeState {
                        id: Sha1(0000000000000000000000000000000000000000),
                        kind: Blob,
                    },
                    flags: Some(
                        ExecutableBitRemoved,
                    ),
                },
            },
            TreeChange {
                path: "file-renamed",
                status: Rename {
                    previous_path: "file-to-be-renamed",
                    previous_state: ChangeState {
                        id: Sha1(3d3b36f021391fa57312d7dfd1ad8cf5a13dca6d),
                        kind: Blob,
                    },
                    state: ChangeState {
                        id: Sha1(0000000000000000000000000000000000000000),
                        kind: BlobExecutable,
                    },
                    flags: Some(
                        ExecutableBitAdded,
                    ),
                },
            },
            TreeChange {
                path: "file-renamed-in-index",
                status: Rename {
                    previous_path: "file-to-be-renamed-in-index",
                    previous_state: ChangeState {
                        id: Sha1(3d3b36f021391fa57312d7dfd1ad8cf5a13dca6d),
                        kind: Blob,
                    },
                    state: ChangeState {
                        id: Sha1(3d3b36f021391fa57312d7dfd1ad8cf5a13dca6d),
                        kind: Blob,
                    },
                    flags: None,
                },
            },
        ],
        ignored_changes: [
            IgnoredWorktreeChange {
                path: "file-in-index",
                status: TreeIndexWorktreeChangeIneffective,
            },
        ],
    }
    "#);

    Ok(())
}

mod util {
    use bstr::BString;
    use but_core::{TreeChange, UnifiedPatch, unified_diff::DiffHunk};
    use gix::prelude::ObjectIdExt;

    pub fn previous_change_text(
        repo: &gix::Repository,
        change: &TreeChange,
    ) -> anyhow::Result<BString> {
        Ok(change
            .status
            .previous_state_and_path()
            .expect("modification")
            .0
            .id
            .attach(repo)
            .object()?
            .detach()
            .data
            .into())
    }

    pub fn changed_file_in_worktree_with_hunks(
        repo: &gix::Repository,
        filename: &str,
        context_lines: u32,
    ) -> anyhow::Result<(TreeChange, Vec<DiffHunk>)> {
        let change = but_core::diff::worktree_changes(repo)?
            .changes
            .into_iter()
            .find(|change| change.path == filename)
            .expect("well-known fixture");

        let Some(UnifiedPatch::Patch { hunks, .. }) = change.unified_patch(repo, context_lines)?
        else {
            unreachable!("We know there are hunks")
        };
        Ok((change, hunks))
    }
}
