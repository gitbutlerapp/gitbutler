use anyhow::{Context, Result};
use but_core::ref_metadata::ProjectMeta;
use but_graph::init::Options;
use but_rebase::graph_rebase::mutate::RelativeTo;
use but_workspace::{
    BottomUpdate, BottomUpdateKind, integrate_upstream, resolve_worktree_conflicts,
    worktree_conflicts_for_rebase,
};

use crate::ref_info::with_workspace_commit::utils::{
    StackState, add_stack, named_writable_scenario_with_description,
};

fn project_meta(repo: &gix::Repository) -> Result<ProjectMeta> {
    Ok(ProjectMeta {
        target_ref: Some("refs/remotes/origin/A".try_into()?),
        target_commit_id: Some(repo.rev_parse_single("main")?.detach()),
        push_remote: None,
    })
}

#[test]
fn conflict_preview_reports_dirty_worktree_paths() -> Result<()> {
    let (_tmp, repo, mut meta, _description, mut db) = upstream_conflict_fixture()?;
    std::fs::write(
        repo.workdir_path("shared.txt").expect("non-bare"),
        "dirty\n",
    )?;
    let mut workspace = workspace_for_stack(&repo, &meta, &mut db)?;

    let project_meta = project_meta(&repo)?;
    let rebase = integrate_upstream(
        &mut workspace,
        &mut meta,
        project_meta,
        &repo,
        &mut db,
        vec![BottomUpdate {
            kind: BottomUpdateKind::Rebase,
            selector: RelativeTo::Commit(repo.rev_parse_single("A")?.detach()),
        }],
    )?
    .rebase;

    let conflicts = worktree_conflicts_for_rebase(&rebase)?;

    assert_eq!(
        conflicts,
        vec![but_serde::BStringForFrontend::from("shared.txt")],
        "dirty worktree changes conflicting with the preview head should be reported"
    );
    Ok(())
}

#[test]
fn conflict_preview_includes_index_conflicts_when_worktree_is_dirty() -> Result<()> {
    let (_tmp, repo, mut meta, _description, mut db) = upstream_conflict_fixture()?;
    std::fs::write(
        repo.workdir_path("shared.txt").expect("non-bare"),
        "staged\n",
    )?;
    git(&repo, ["add", "shared.txt"])?;
    std::fs::write(
        repo.workdir_path("unrelated.txt").expect("non-bare"),
        "dirty\n",
    )?;
    let mut workspace = workspace_for_stack(&repo, &meta, &mut db)?;

    let project_meta = project_meta(&repo)?;
    let rebase = integrate_upstream(
        &mut workspace,
        &mut meta,
        project_meta,
        &repo,
        &mut db,
        vec![BottomUpdate {
            kind: BottomUpdateKind::Rebase,
            selector: RelativeTo::Commit(repo.rev_parse_single("A")?.detach()),
        }],
    )?
    .rebase;

    let conflicts = worktree_conflicts_for_rebase(&rebase)?;

    assert_eq!(
        conflicts,
        vec![but_serde::BStringForFrontend::from("shared.txt")],
        "staged changes should be checked even when unstaged worktree changes are present"
    );
    Ok(())
}

#[test]
fn conflict_preview_uses_rebase_repo_for_preview_objects() -> Result<()> {
    let (_tmp, repo, mut meta, _description, mut db) = upstream_conflict_fixture()?;
    std::fs::write(
        repo.workdir_path("shared.txt").expect("non-bare"),
        "dirty\n",
    )?;
    let mut workspace = workspace_for_stack(&repo, &meta, &mut db)?;

    let project_meta = project_meta(&repo)?;
    let rebase = integrate_upstream(
        &mut workspace,
        &mut meta,
        project_meta,
        &repo,
        &mut db,
        vec![BottomUpdate {
            kind: BottomUpdateKind::Rebase,
            selector: RelativeTo::Commit(repo.rev_parse_single("A")?.detach()),
        }],
    )?
    .rebase;

    let preview_workspace = rebase.overlayed_graph()?.into_workspace()?;
    let preview_head = preview_workspace
        .graph
        .entrypoint()?
        .commit()
        .context("preview workspace should have a head commit")?
        .id;
    assert!(
        repo.find_object(preview_head).is_err(),
        "preview commits should not have to exist in the persistent repository before materialization"
    );

    let conflicts = worktree_conflicts_for_rebase(&rebase)?;

    assert_eq!(
        conflicts,
        vec![but_serde::BStringForFrontend::from("shared.txt")],
        "conflict preview should read rewritten objects from the rebase repository"
    );
    Ok(())
}

#[test]
fn conflict_preview_returns_empty_for_non_conflicting_dirty_worktree() -> Result<()> {
    let (_tmp, repo, mut meta, _description, mut db) = upstream_conflict_fixture()?;
    std::fs::write(
        repo.workdir_path("unrelated.txt").expect("non-bare"),
        "dirty\n",
    )?;
    let mut workspace = workspace_for_stack(&repo, &meta, &mut db)?;

    let project_meta = project_meta(&repo)?;
    let rebase = integrate_upstream(
        &mut workspace,
        &mut meta,
        project_meta,
        &repo,
        &mut db,
        vec![BottomUpdate {
            kind: BottomUpdateKind::Rebase,
            selector: RelativeTo::Commit(repo.rev_parse_single("A")?.detach()),
        }],
    )?
    .rebase;

    let conflicts = worktree_conflicts_for_rebase(&rebase)?;

    assert!(
        conflicts.is_empty(),
        "dirty worktree changes that merge cleanly should not be reported"
    );
    Ok(())
}

#[test]
fn conflict_preview_returns_empty_for_ignored_only_worktree_changes() -> Result<()> {
    let (_tmp, repo, mut meta, _description, mut db) = upstream_conflict_fixture()?;
    std::fs::write(repo.git_dir().join("info/exclude"), "ignored.txt\n")?;
    std::fs::write(
        repo.workdir_path("ignored.txt").expect("non-bare"),
        "ignored\n",
    )?;
    let mut workspace = workspace_for_stack(&repo, &meta, &mut db)?;

    let project_meta = project_meta(&repo)?;
    let rebase = integrate_upstream(
        &mut workspace,
        &mut meta,
        project_meta,
        &repo,
        &mut db,
        vec![BottomUpdate {
            kind: BottomUpdateKind::Rebase,
            selector: RelativeTo::Commit(repo.rev_parse_single("A")?.detach()),
        }],
    )?
    .rebase;

    let conflicts = worktree_conflicts_for_rebase(&rebase)?;

    assert!(
        conflicts.is_empty(),
        "ignored-only changes cannot be represented in the snapshot and should be a no-op"
    );
    Ok(())
}

#[test]
fn resolve_worktree_conflict_takes_worktree_content() -> Result<()> {
    let (_tmp, repo, _meta, _description, _db) = upstream_conflict_fixture()?;
    make_index_conflict(&repo)?;
    std::fs::write(
        repo.workdir_path("shared.txt").expect("non-bare"),
        "resolved\n",
    )?;

    resolve_worktree_conflicts(&repo, ["shared.txt".into()])?;

    let index = repo.index()?;
    let entries: Vec<_> = index
        .prefixed_entries("shared.txt".into())
        .unwrap_or_default()
        .iter()
        .map(|e| (e.stage(), e.id))
        .collect();
    assert_eq!(
        entries,
        vec![(
            gix::index::entry::Stage::Unconflicted,
            repo.write_blob("resolved\n")?.detach()
        )],
        "only a stage-0 entry with the worktree content remains"
    );

    let changes = but_core::diff::worktree_changes(&repo)?;
    assert!(changes.index_conflicts.is_empty(), "the conflict is gone");
    assert_eq!(
        changes
            .changes
            .iter()
            .map(|c| c.path.to_string())
            .collect::<Vec<_>>(),
        ["shared.txt"],
        "the file is now an ordinary uncommitted change"
    );
    assert_eq!(
        changes.changes[0].status.kind(),
        but_core::TreeStatusKind::Modification,
        "resolved with different content than HEAD"
    );
    Ok(())
}

#[test]
fn resolve_worktree_conflict_of_deleted_file_removes_all_stages() -> Result<()> {
    let (_tmp, repo, _meta, _description, _db) = upstream_conflict_fixture()?;
    make_index_conflict(&repo)?;
    std::fs::remove_file(repo.workdir_path("shared.txt").expect("non-bare"))?;

    resolve_worktree_conflicts(&repo, ["shared.txt".into()])?;

    let index = repo.index()?;
    assert!(
        index.prefixed_entries("shared.txt".into()).is_none(),
        "no index entry is left for a file that was deleted to resolve"
    );
    let changes = but_core::diff::worktree_changes(&repo)?;
    assert!(changes.index_conflicts.is_empty(), "the conflict is gone");
    assert_eq!(
        changes
            .changes
            .iter()
            .map(|c| c.path.to_string())
            .collect::<Vec<_>>(),
        ["shared.txt"],
        "the file is now an ordinary uncommitted change"
    );
    assert_eq!(
        changes.changes[0].status.kind(),
        but_core::TreeStatusKind::Deletion,
        "the file is tracked in HEAD but gone from the worktree"
    );
    Ok(())
}

#[test]
fn resolve_worktree_conflict_refuses_unconflicted_path() -> Result<()> {
    let (_tmp, repo, _meta, _description, _db) = upstream_conflict_fixture()?;
    let err = resolve_worktree_conflicts(&repo, ["shared.txt".into()]).unwrap_err();
    assert_eq!(err.to_string(), "'shared.txt' has no unresolved conflict");
    Ok(())
}

/// Leave behind what a conflicting checkout produces for `shared.txt`: unmerged
/// entries in the index.
fn make_index_conflict(repo: &gix::Repository) -> Result<()> {
    git(repo, ["update-index", "--refresh"])?;
    git(repo, ["read-tree", "-m", "-u", "main", "A", "new-origin"])
}

fn upstream_conflict_fixture() -> Result<(
    but_testsupport::gix_testtools::tempfile::TempDir,
    gix::Repository,
    but_meta::VirtualBranchesTomlMetadata,
    String,
    but_db::DbHandle,
)> {
    let (tmp, repo, mut meta, description, db) =
        named_writable_scenario_with_description("remote-diverged-with-workspace-conflicting")?;
    add_stack(&mut meta, 1, "A", StackState::InWorkspace);

    Ok((tmp, repo, meta, description, db))
}

fn workspace_for_stack(
    repo: &gix::Repository,
    meta: &but_meta::VirtualBranchesTomlMetadata,
    db: &mut but_db::DbHandle,
) -> Result<but_graph::Workspace> {
    let target_sha = repo.rev_parse_single("main")?.detach();
    let ws = but_graph::Graph::from_head(
        repo,
        meta,
        project_meta(repo)?,
        db,
        Options {
            extra_target_commit_id: Some(target_sha),
            ..Options::limited()
        },
    )?
    .into_workspace()?;
    Ok(ws)
}

fn git(
    repo: &gix::Repository,
    args: impl IntoIterator<Item = impl AsRef<std::ffi::OsStr>>,
) -> Result<()> {
    let status = std::process::Command::new("git")
        .current_dir(repo.workdir().expect("writable scenarios are non-bare"))
        .args(args)
        .status()?;
    assert!(status.success(), "git command should succeed");
    Ok(())
}
