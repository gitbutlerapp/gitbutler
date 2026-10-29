use anyhow::Result;
use but_core::DiffSpec;
use but_testsupport::git_status_at_dir;
use but_workspace::worktrees::{move_uncommitted_changes, open_worktree_repo};

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

    let outcome = move_uncommitted_changes(&repo, &wt_repo, None, 0)?;
    assert!(!outcome.conflict_occurred);

    assert_eq!(
        std::fs::read_to_string(main_dir.join("shared"))?,
        "line1-wt\nline2\nline3\nline4\nline5\nline6\nline7\n",
        "the worktree's tracked modification landed in main"
    );
    assert_eq!(
        std::fs::read_to_string(main_dir.join("wt-file"))?,
        "wt-only\n",
        "the worktree's untracked addition landed in main"
    );
    assert_eq!(
        std::fs::read_to_string(main_dir.join("main-file"))?,
        "main-only-edited\n",
        "main's own unrelated uncommitted change survived the merge untouched"
    );

    let wt_status = git_status_at_dir(&wt_dir)?;
    assert_eq!(
        wt_status, "",
        "a clean move clears everything it moved out of the worktree: {wt_status}"
    );
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

    let outcome = move_uncommitted_changes(&repo, &wt_repo, None, 0)?;
    assert!(outcome.conflict_occurred);

    let merged = std::fs::read_to_string(main_dir.join("shared"))?;
    assert!(
        merged.contains("<<<<<<<") && merged.contains("=======") && merged.contains(">>>>>>>"),
        "conflict markers were written into main's working directory: {merged}"
    );
    assert!(
        merged.contains("line1-wt"),
        "the worktree's pre-move content survives verbatim inside the conflict markers: {merged}"
    );
    assert_eq!(
        std::fs::read_to_string(main_dir.join("wt-file"))?,
        "wt-only\n",
        "the non-conflicting untracked addition still moved over"
    );

    assert_eq!(
        git_status_at_dir(&wt_dir)?,
        "",
        "the worktree is cleared even though the destination conflicted - its content isn't \
         lost, it's now inside the conflict markers in main"
    );
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

    assert_eq!(
        std::fs::read_to_string(main_dir.join("shared"))?,
        "line1-wt\nline2\nline3\nline4\nline5\nline6\nline7\n",
        "only the selected hunk landed in main"
    );
    assert_eq!(
        std::fs::read_to_string(wt_dir.join("shared"))?,
        "line1\nline2\nline3\nline4\nline5\nline6\nline7-wt\n",
        "the unselected hunk remains dirty in the worktree"
    );
    assert!(
        !main_dir.join("wt-file").exists(),
        "the untracked addition wasn't selected, so it wasn't moved"
    );
    assert!(
        wt_dir.join("wt-file").exists(),
        "the untracked addition wasn't selected, so it stays in the worktree"
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
    assert_eq!(err.to_string(), "No changes to move");
    Ok(())
}

#[test]
fn moving_an_empty_selection_is_an_error() -> Result<()> {
    let (repo, _tmp) = scenario();
    let wt_repo = open_worktree_repo(&repo, "wt".into())?;

    let err = move_uncommitted_changes(&repo, &wt_repo, Some(Vec::new()), 0).unwrap_err();
    assert_eq!(err.to_string(), "No changes were selected to move");
    Ok(())
}
