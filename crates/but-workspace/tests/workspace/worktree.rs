use anyhow::{Context, Result};
use but_meta::virtual_branches_legacy_types::Target;
use but_rebase::graph_rebase::mutate::RelativeTo;
use but_workspace::{
    BottomUpdate, BottomUpdateKind, integrate_upstream, worktree_conflicts_for_rebase,
};
use gix::prelude::ObjectIdExt;

use crate::ref_info::with_workspace_commit::utils::{
    StackState, add_stack, add_stack_with_segments, named_writable_scenario_with_description,
};

fn project_meta(meta: &impl but_core::RefMetadata) -> Result<but_core::ref_metadata::ProjectMeta> {
    Ok(meta
        .workspace(but_core::WORKSPACE_REF_NAME.try_into()?)?
        .project_meta())
}

#[test]
fn conflict_preview_reports_dirty_worktree_paths() -> Result<()> {
    let (_tmp, repo, mut meta, _description) = upstream_conflict_fixture()?;
    std::fs::write(
        repo.workdir_path("shared.txt").expect("non-bare"),
        "dirty\n",
    )?;
    let mut workspace = workspace_for_stack(&repo, &meta)?;

    let project_meta = project_meta(&meta)?;
    let rebase = integrate_upstream(
        &mut workspace,
        &mut meta,
        project_meta,
        &repo,
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
fn conflict_preview_uses_direct_stack_checkout() -> Result<()> {
    let (_tmp, repo, mut meta, _description) =
        named_writable_scenario_with_description("partially-integrated-multi-branch-stack")?;
    let target_sha = repo.rev_parse_single("main")?.detach();
    meta.data_mut().default_target = Some(Target {
        branch: gitbutler_reference::RemoteRefname::new("origin", "main"),
        remote_url: "should not be needed and when it is extract it from `repo`".to_string(),
        sha: target_sha,
        push_remote_name: None,
    });
    add_stack_with_segments(&mut meta, 1, "A", StackState::InWorkspace, &["C"]);
    add_stack(&mut meta, 2, "B", StackState::InWorkspace);
    git(&repo, ["checkout", "A"])?;
    std::fs::write(repo.workdir_path("B.txt").expect("non-bare"), "dirty\n")?;
    let mut workspace = workspace_for_stack(&repo, &meta)?;

    let project_meta = project_meta(&meta)?;
    let rebase = integrate_upstream(
        &mut workspace,
        &mut meta,
        project_meta,
        &repo,
        vec![BottomUpdate {
            kind: BottomUpdateKind::Rebase,
            selector: RelativeTo::Commit(repo.rev_parse_single("A")?.detach()),
        }],
    )?
    .rebase;

    let preview_workspace = rebase.overlayed_workspace()?;
    let entrypoint = entrypoint_commit_id(&preview_workspace)
        .context("direct checkout should have a preview entrypoint commit")?;
    let containing_workspace = crate::utils::workspace_tip_id(&preview_workspace)
        .context("direct checkout should still resolve its containing workspace")?;
    assert_ne!(
        entrypoint, containing_workspace,
        "the direct checkout must not be replaced by its containing workspace"
    );
    let entrypoint_tree = but_core::Commit::from_id(entrypoint.attach(rebase.repo()))?
        .tree_id_or_auto_resolution()?;
    let containing_workspace_tree =
        but_core::Commit::from_id(containing_workspace.attach(rebase.repo()))?
            .tree_id_or_auto_resolution()?;
    assert_ne!(
        entrypoint_tree, containing_workspace_tree,
        "the fixture must distinguish the direct checkout from its containing workspace by content"
    );

    let conflicts = worktree_conflicts_for_rebase(&rebase)?;
    assert!(
        conflicts.is_empty(),
        "a dirty path absent from the direct checkout must not conflict merely because it exists in the containing workspace"
    );
    Ok(())
}

#[test]
fn conflict_preview_includes_index_conflicts_when_worktree_is_dirty() -> Result<()> {
    let (_tmp, repo, mut meta, _description) = upstream_conflict_fixture()?;
    std::fs::write(
        repo.workdir_path("shared.txt").expect("non-bare"),
        "staged\n",
    )?;
    git(&repo, ["add", "shared.txt"])?;
    std::fs::write(
        repo.workdir_path("unrelated.txt").expect("non-bare"),
        "dirty\n",
    )?;
    let mut workspace = workspace_for_stack(&repo, &meta)?;

    let project_meta = project_meta(&meta)?;
    let rebase = integrate_upstream(
        &mut workspace,
        &mut meta,
        project_meta,
        &repo,
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
    let (_tmp, repo, mut meta, _description) = upstream_conflict_fixture()?;
    std::fs::write(
        repo.workdir_path("shared.txt").expect("non-bare"),
        "dirty\n",
    )?;
    let mut workspace = workspace_for_stack(&repo, &meta)?;

    let project_meta = project_meta(&meta)?;
    let rebase = integrate_upstream(
        &mut workspace,
        &mut meta,
        project_meta,
        &repo,
        vec![BottomUpdate {
            kind: BottomUpdateKind::Rebase,
            selector: RelativeTo::Commit(repo.rev_parse_single("A")?.detach()),
        }],
    )?
    .rebase;

    let preview_workspace = rebase.overlayed_workspace()?;
    let preview_head = crate::utils::workspace_tip_id(&preview_workspace)
        .context("preview workspace should have a head commit")?;
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
    let (_tmp, repo, mut meta, _description) = upstream_conflict_fixture()?;
    std::fs::write(
        repo.workdir_path("unrelated.txt").expect("non-bare"),
        "dirty\n",
    )?;
    let mut workspace = workspace_for_stack(&repo, &meta)?;

    let project_meta = project_meta(&meta)?;
    let rebase = integrate_upstream(
        &mut workspace,
        &mut meta,
        project_meta,
        &repo,
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
    let (_tmp, repo, mut meta, _description) = upstream_conflict_fixture()?;
    std::fs::write(repo.git_dir().join("info/exclude"), "ignored.txt\n")?;
    std::fs::write(
        repo.workdir_path("ignored.txt").expect("non-bare"),
        "ignored\n",
    )?;
    let mut workspace = workspace_for_stack(&repo, &meta)?;

    let project_meta = project_meta(&meta)?;
    let rebase = integrate_upstream(
        &mut workspace,
        &mut meta,
        project_meta,
        &repo,
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

fn upstream_conflict_fixture() -> Result<(
    but_testsupport::gix_testtools::tempfile::TempDir,
    gix::Repository,
    but_meta::VirtualBranchesTomlMetadata,
    String,
)> {
    let (tmp, repo, mut meta, description) =
        named_writable_scenario_with_description("remote-diverged-with-workspace-conflicting")?;
    let target_sha = repo.rev_parse_single("main")?.detach();

    meta.data_mut().default_target = Some(Target {
        branch: gitbutler_reference::RemoteRefname::new("origin", "A"),
        remote_url: "should not be needed and when it is extract it from `repo`".to_string(),
        sha: target_sha,
        push_remote_name: None,
    });
    add_stack(&mut meta, 1, "A", StackState::InWorkspace);

    Ok((tmp, repo, meta, description))
}

fn workspace_for_stack(
    repo: &gix::Repository,
    meta: &but_meta::VirtualBranchesTomlMetadata,
) -> Result<but_graph::Workspace> {
    let target_sha = repo.rev_parse_single("main")?.detach();
    let mut project_meta = project_meta(meta)?;
    project_meta.target_commit_id = Some(target_sha);
    let ws = but_graph::Graph::from_repo(
        repo,
        meta,
        project_meta,
        but_graph::init::Overlay::default(),
    )?
    .into_workspace()?;
    Ok(ws)
}

fn entrypoint_commit_id(workspace: &but_graph::Workspace) -> Option<gix::ObjectId> {
    match workspace.graph.entrypoint() {
        but_graph::NodeGraphEntrypoint::Node(index) => {
            let but_graph::NodeKind::Commit { id } = workspace.graph.nodes().get(*index)?.kind()
            else {
                unreachable!("born graph entrypoints are commits")
            };
            Some(*id)
        }
        but_graph::NodeGraphEntrypoint::Unborn(_) => None,
    }
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
