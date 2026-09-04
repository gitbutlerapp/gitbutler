use anyhow::Result;
use but_core::DiffSpec;
use but_testsupport::git_status_at_dir;
use but_workspace::worktrees::{move_uncommitted_changes, open_worktree_repo};
use snapbox::str;

use crate::utils::writable_scenario_slow;

fn scenario() -> (
    gix::Repository,
    but_testsupport::gix_testtools::tempfile::TempDir,
) {
    writable_scenario_slow("worktree-move-uncommitted")
}

#[test]
fn full_move_preserves_unrelated_main_changes_and_clears_the_worktree() -> Result<()> {
    let (repo, _tmp) = scenario();
    let main_dir = repo.workdir().expect("non-bare");
    let wt_repo = open_worktree_repo(&repo, "wt".into())?;
    let wt_dir = wt_repo.workdir().expect("non-bare").to_owned();

    // The worktree's own uncommitted changes: a tracked modification and an untracked addition.
    std::fs::write(
        wt_dir.join("shared"),
        "line1-wt\nline2\nline3\nline4\nline5\nline6\nline7\n",
    )?;
    // Main's own, unrelated uncommitted change, which the move must not disturb.
    std::fs::write(main_dir.join("main-file"), "main-only-edited\n")?;
    snapbox::assert_data_eq!(
        git_status_at_dir(main_dir)?,
        str![[r#"
 M main-file
?? wt/

"#]]
    );
    snapbox::assert_data_eq!(
        git_status_at_dir(&wt_dir)?,
        str![[r#"
 M shared
?? wt-file

"#]]
    );

    let outcome = move_uncommitted_changes(&repo, &wt_repo, None, 0)?;
    assert!(!outcome.conflict_occurred);

    // Both worktree changes are staged in main while main's own edit is untouched on disk.
    // Note the index: it was rewritten to the moved tree, which never contained `main-file`,
    // so the file HEAD tracks shows as a staged deletion next to its untracked on-disk edit.
    snapbox::assert_data_eq!(
        git_status_at_dir(main_dir)?,
        str![[r#"
D  main-file
M  shared
A  wt-file
?? main-file
?? wt/

"#]]
    );
    snapbox::assert_data_eq!(
        std::fs::read_to_string(main_dir.join("shared"))?,
        str![[r#"
line1-wt
line2
line3
line4
line5
line6
line7

"#]]
    );
    snapbox::assert_data_eq!(
        std::fs::read_to_string(main_dir.join("wt-file"))?,
        str![[r#"
wt-only

"#]]
    );
    snapbox::assert_data_eq!(
        std::fs::read_to_string(main_dir.join("main-file"))?,
        str![[r#"
main-only-edited

"#]]
    );
    // A clean move clears everything it moved out of the worktree.
    snapbox::assert_data_eq!(git_status_at_dir(&wt_dir)?, str![[r#""#]]);
    Ok(())
}

#[test]
fn conflicting_move_writes_markers_into_main_and_still_clears_the_worktree() -> Result<()> {
    let (repo, _tmp) = scenario();
    let main_dir = repo.workdir().expect("non-bare");
    let wt_repo = open_worktree_repo(&repo, "wt".into())?;
    let wt_dir = wt_repo.workdir().expect("non-bare").to_owned();

    // Both sides edit the exact same line differently: a genuine, expected conflict.
    std::fs::write(
        wt_dir.join("shared"),
        "line1-wt\nline2\nline3\nline4\nline5\nline6\nline7\n",
    )?;
    std::fs::write(
        main_dir.join("shared"),
        "line1-main\nline2\nline3\nline4\nline5\nline6\nline7\n",
    )?;
    snapbox::assert_data_eq!(
        git_status_at_dir(main_dir)?,
        str![[r#"
 M shared
?? wt/

"#]]
    );
    snapbox::assert_data_eq!(
        git_status_at_dir(&wt_dir)?,
        str![[r#"
 M shared
?? wt-file

"#]]
    );

    let outcome = move_uncommitted_changes(&repo, &wt_repo, None, 0)?;
    assert!(outcome.conflict_occurred);

    // The worktree's pre-move content survives verbatim inside the conflict markers; `main-file`
    // shows the same index rewrite as in the clean move above.
    snapbox::assert_data_eq!(
        git_status_at_dir(main_dir)?,
        str![[r#"
D  main-file
UU shared
A  wt-file
?? main-file
?? wt/

"#]]
    );
    snapbox::assert_data_eq!(
        std::fs::read_to_string(main_dir.join("shared"))?,
        str![[r#"
<<<<<<< ours
line1-main
=======
line1-wt
>>>>>>> theirs
line2
line3
line4
line5
line6
line7

"#]]
    );
    snapbox::assert_data_eq!(
        std::fs::read_to_string(main_dir.join("wt-file"))?,
        str![[r#"
wt-only

"#]]
    );
    // The worktree is cleared even though the destination conflicted - its content isn't
    // lost, it's now inside the conflict markers in main.
    snapbox::assert_data_eq!(git_status_at_dir(&wt_dir)?, str![[r#""#]]);
    Ok(())
}

#[test]
fn selecting_one_hunk_leaves_the_other_dirty_in_the_worktree() -> Result<()> {
    let (repo, _tmp) = scenario();
    let main_dir = repo.workdir().expect("non-bare");
    let wt_repo = open_worktree_repo(&repo, "wt".into())?;
    let wt_dir = wt_repo.workdir().expect("non-bare").to_owned();

    // Two independent hunks, far apart, in the same file.
    std::fs::write(
        wt_dir.join("shared"),
        "line1-wt\nline2\nline3\nline4\nline5\nline6\nline7-wt\n",
    )?;
    snapbox::assert_data_eq!(
        git_status_at_dir(main_dir)?,
        str![[r#"
?? wt/

"#]]
    );
    snapbox::assert_data_eq!(
        git_status_at_dir(&wt_dir)?,
        str![[r#"
 M shared
?? wt-file

"#]]
    );

    let change = but_core::diff::worktree_changes(&wt_repo)?
        .changes
        .into_iter()
        .find(|change| change.path == "shared")
        .expect("shared is modified");
    let but_core::UnifiedPatch::Patch { hunks, .. } = change
        .unified_patch(&wt_repo, 0)?
        .expect("text changes have a patch")
    else {
        panic!("text changes have a patch")
    };
    assert_eq!(
        hunks.len(),
        2,
        "the fixture edit produced two selectable hunks"
    );
    let selection = vec![DiffSpec {
        path: "shared".into(),
        hunk_headers: vec![(&hunks[0]).into()],
        ..Default::default()
    }];

    let outcome = move_uncommitted_changes(&repo, &wt_repo, Some(selection), 0)?;
    assert!(!outcome.conflict_occurred);

    // Only the selected hunk landed in main; the unselected untracked file didn't move, and
    // `main-file` shows the same index rewrite as in the full move.
    snapbox::assert_data_eq!(
        git_status_at_dir(main_dir)?,
        str![[r#"
D  main-file
M  shared
?? main-file
?? wt/

"#]]
    );
    snapbox::assert_data_eq!(
        std::fs::read_to_string(main_dir.join("shared"))?,
        str![[r#"
line1-wt
line2
line3
line4
line5
line6
line7

"#]]
    );
    // The unselected hunk and the untracked file remain dirty in the worktree; the hunk is
    // staged there because the checkout rewrote the worktree's index as well.
    snapbox::assert_data_eq!(
        git_status_at_dir(&wt_dir)?,
        str![[r#"
M  shared
?? wt-file

"#]]
    );
    snapbox::assert_data_eq!(
        std::fs::read_to_string(wt_dir.join("shared"))?,
        str![[r#"
line1
line2
line3
line4
line5
line6
line7-wt

"#]]
    );
    Ok(())
}

#[test]
fn moving_nothing_is_an_error() -> Result<()> {
    let (repo, _tmp) = scenario();
    let wt_repo = open_worktree_repo(&repo, "wt".into())?;
    let wt_dir = wt_repo.workdir().expect("non-bare").to_owned();
    // The fixture's own untracked addition is the worktree's only dirty state; remove it too.
    std::fs::remove_file(wt_dir.join("wt-file"))?;

    let err = move_uncommitted_changes(&repo, &wt_repo, None, 0).unwrap_err();
    snapbox::assert_data_eq!(format!("{err:#}"), str!["No changes to move"]);
    Ok(())
}

#[test]
fn moving_an_empty_selection_is_an_error() -> Result<()> {
    let (repo, _tmp) = scenario();
    let wt_repo = open_worktree_repo(&repo, "wt".into())?;

    let err = move_uncommitted_changes(&repo, &wt_repo, Some(Vec::new()), 0).unwrap_err();
    snapbox::assert_data_eq!(format!("{err:#}"), str!["No changes were selected to move"]);
    Ok(())
}
