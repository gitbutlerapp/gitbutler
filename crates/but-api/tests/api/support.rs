use but_core::ref_metadata::ProjectMeta;
use but_rebase::graph_rebase::mutate::InsertSide;
use but_testsupport::gix_testtools::tempfile::TempDir;
use but_testsupport::{CommandExt, git_at_dir, open_repo};

pub fn writable_scenario(name: &str) -> (gix::Repository, TempDir) {
    but_testsupport::writable_scenario(name)
}

/// Build a minimal ad-hoc repository: `main` with two commits, a `feature` branch at the first
/// commit, and a `refs/remotes/origin/main` remote-tracking ref, with `HEAD` on `main`.
pub fn repo_with_feature_branch() -> anyhow::Result<(gix::Repository, tempfile::TempDir)> {
    let tmp = tempfile::tempdir()?;
    git_at_dir(tmp.path()).args(["init"]).run();
    git_at_dir(tmp.path())
        .args(["config", "user.name", "GitButler"])
        .run();
    git_at_dir(tmp.path())
        .args(["config", "user.email", "gitbutler@example.com"])
        .run();
    write_file(tmp.path(), "file.txt", "one\n")?;
    git_at_dir(tmp.path()).args(["add", "file.txt"]).run();
    git_at_dir(tmp.path()).args(["commit", "-m", "one"]).run();
    git_at_dir(tmp.path()).args(["branch", "feature"]).run();
    git_at_dir(tmp.path())
        .args(["config", "remote.origin.url", "../origin"])
        .run();
    git_at_dir(tmp.path())
        .args(["update-ref", "refs/remotes/origin/main", "HEAD"])
        .run();
    write_file(tmp.path(), "file.txt", "two\n")?;
    git_at_dir(tmp.path()).args(["commit", "-am", "two"]).run();

    Ok((open_repo(tmp.path())?, tmp))
}

/// Persist a project default target pointing at `origin/main` but at the `feature` branch's commit.
pub fn set_project_target_to_feature(repo: &gix::Repository) -> anyhow::Result<gix::ObjectId> {
    let mut feature = repo.find_reference("refs/heads/feature")?;
    let target_commit_id = feature.peel_to_id()?.detach();
    ProjectMeta {
        target_ref: Some("refs/remotes/origin/main".try_into()?),
        target_commit_id: Some(target_commit_id),
        push_remote: Some("origin".into()),
    }
    .persist_to_local_config(repo)?;
    Ok(target_commit_id)
}

/// Add a linked worktree with `branch` checked out, and return the temp dir holding it (kept alive
/// by the caller to preserve the checkout). Used to exercise the "checked out elsewhere" guard.
pub fn checkout_branch_in_linked_worktree(
    main_worktree: &std::path::Path,
    branch: &str,
) -> anyhow::Result<TempDir> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("wt");
    git_at_dir(main_worktree)
        .args([
            "worktree",
            "add",
            path.to_str().expect("utf-8 path"),
            branch,
        ])
        .run();
    Ok(dir)
}

pub fn write_file(
    root: &std::path::Path,
    relative_path: &str,
    content: &str,
) -> anyhow::Result<()> {
    std::fs::write(root.join(relative_path), content)?;
    Ok(())
}

/// Create `new_ref` as an empty branch directly above `anchor` through the create API, mirroring
/// the ad-hoc "create dependent branch above" flow (which also checks the new branch out).
pub fn create_empty_branch_above(
    ctx: &mut but_ctx::Context,
    new_ref: &gix::refs::FullName,
    anchor: &gix::refs::FullName,
) -> anyhow::Result<()> {
    but_api::branch::branch_create(
        ctx,
        Some(new_ref.clone()),
        but_api::branch::json::BranchCreatePlacement::Dependent {
            relative_to: but_api::commit::json::RelativeTo::Reference(anchor.clone()),
            side: InsertSide::Above,
        },
    )?;
    Ok(())
}

/// Assert the mutation response's workspace projection contains `expected` as its checked-out ref,
/// in whichever projection flavor this build uses.
#[cfg(not(feature = "graph-workspace"))]
pub fn assert_workspace_ref(workspace: &but_api::WorkspaceState, expected: &str) {
    let workspace_ref = workspace
        .head_info
        .workspace_ref_info
        .as_ref()
        .expect("checked out branch is the workspace ref");
    assert_eq!(workspace_ref.ref_name.as_bstr(), expected);
}

/// Assert the mutation response's workspace projection contains `expected` as its checked-out ref,
/// in whichever projection flavor this build uses.
#[cfg(feature = "graph-workspace")]
pub fn assert_workspace_ref(workspace: &but_api::WorkspaceState, expected: &str) {
    use but_workspace::ui::workspace::DetailedGraphRowData;
    assert!(
        workspace.graph_workspace.stacks.iter().any(|stack| {
            stack.rows.iter().any(|row| {
                matches!(
                    &row.data,
                    DetailedGraphRowData::Reference(reference)
                        if reference.ref_name.full_name == expected
                )
            })
        }),
        "checked out branch '{expected}' appears in the graph workspace"
    );
}

pub fn persist_default_target(repo: &gix::Repository) -> anyhow::Result<gix::ObjectId> {
    let target_commit_id = repo.rev_parse_single("refs/heads/main")?.detach();
    ProjectMeta {
        target_ref: Some("refs/remotes/origin/main".try_into()?),
        target_commit_id: Some(target_commit_id),
        push_remote: Some("origin".into()),
    }
    .persist_to_local_config(repo)?;
    Ok(target_commit_id)
}

pub fn repository_graph(repo: &gix::Repository) -> anyhow::Result<String> {
    Ok(but_testsupport::visualize_commit_graph_all(repo)?)
}

pub fn workspace_graph(ctx: &but_ctx::Context) -> anyhow::Result<String> {
    let (_guard, _repo, ws, _db) = ctx.workspace_and_db()?;
    Ok(but_testsupport::graph_workspace(&ws).to_string())
}

#[cfg(not(feature = "graph-workspace"))]
pub fn fresh_head_info(ctx: &but_ctx::Context) -> anyhow::Result<but_workspace::RefInfo> {
    let project_meta = ctx.project_meta()?;
    let meta = ctx.meta()?;
    let repo = ctx.repo.get()?;
    but_workspace::head_info(
        &repo,
        &meta,
        but_workspace::ref_info::Options {
            project_meta,
            traversal: but_graph::init::Options::limited(),
            expensive_commit_info: true,
            ..Default::default()
        },
    )
    .map(but_workspace::RefInfo::pruned_to_entrypoint)
}

#[cfg(feature = "graph-workspace")]
pub fn fresh_graph_workspace(
    ctx: &but_ctx::Context,
) -> anyhow::Result<but_workspace::ui::workspace::DetailedGraphWorkspace> {
    let mut meta = ctx.meta()?;
    let (_guard, repo, ws, _db) = ctx.workspace_and_db()?;
    let mut ws = ws.clone();
    but_workspace::workspace::detailed_graph_workspace(&mut ws, &mut meta, &repo).map(Into::into)
}
