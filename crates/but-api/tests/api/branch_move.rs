use but_core::{DryRun, RefMetadata, ref_metadata::ProjectMeta};
use but_testsupport::{CommandExt, git_at_dir, open_repo};

use crate::support::{
    assert_workspace_ref, create_empty_branch_above, repo_with_feature_branch, write_file,
};

fn context_with_three_branch_stack_options(
    empty_top_branch: bool,
) -> anyhow::Result<(but_ctx::Context, tempfile::TempDir)> {
    let tmp = tempfile::tempdir()?;
    git_at_dir(tmp.path()).args(["init", "-b", "main"]).run();
    git_at_dir(tmp.path())
        .args(["config", "user.name", "GitButler"])
        .run();
    git_at_dir(tmp.path())
        .args(["config", "user.email", "gitbutler@example.com"])
        .run();
    write_file(tmp.path(), "base.txt", "base\n")?;
    git_at_dir(tmp.path()).args(["add", "base.txt"]).run();
    git_at_dir(tmp.path()).args(["commit", "-m", "base"]).run();
    git_at_dir(tmp.path())
        .args(["config", "remote.origin.url", "../origin"])
        .run();
    git_at_dir(tmp.path())
        .args(["update-ref", "refs/remotes/origin/main", "HEAD"])
        .run();

    for branch in ["A", "B", "C"] {
        git_at_dir(tmp.path())
            .args(["checkout", "-b", branch])
            .run();
        if empty_top_branch && branch == "C" {
            continue;
        }
        let file_name = format!("{branch}.txt");
        write_file(tmp.path(), &file_name, &format!("{branch}\n"))?;
        git_at_dir(tmp.path()).args(["add", &file_name]).run();
        git_at_dir(tmp.path()).args(["commit", "-m", branch]).run();
    }

    let repo = open_repo(tmp.path())?;
    let target_commit_id = repo.rev_parse_single("refs/remotes/origin/main")?.detach();
    ProjectMeta {
        target_ref: Some("refs/remotes/origin/main".try_into()?),
        target_commit_id: Some(target_commit_id),
        push_remote: Some("origin".into()),
    }
    .persist(&repo)?;

    let ctx = but_ctx::Context::from_repo_for_testing(repo)?.with_memory_app_cache();
    let mut meta = ctx.meta()?;
    meta.set_branch_stack_order(&[
        "refs/heads/C".try_into()?,
        "refs/heads/B".try_into()?,
        "refs/heads/A".try_into()?,
    ])?;
    drop(meta);
    Ok((ctx, tmp))
}

fn context_with_three_branch_stack() -> anyhow::Result<(but_ctx::Context, tempfile::TempDir)> {
    context_with_three_branch_stack_options(false)
}

fn context_with_empty_top_branch() -> anyhow::Result<(but_ctx::Context, tempfile::TempDir)> {
    context_with_three_branch_stack_options(true)
}

#[cfg(not(feature = "graph-workspace"))]
fn workspace_branch_names(workspace: &but_api::WorkspaceState) -> Vec<String> {
    workspace
        .head_info
        .stacks
        .iter()
        .flat_map(|stack| &stack.segments)
        .filter_map(|segment| {
            segment
                .ref_info
                .as_ref()
                .map(|reference| reference.ref_name.shorten().to_string())
        })
        .collect()
}

#[test]
fn move_non_empty_branch_dry_run_previews_new_tip_without_mutating_repository() -> anyhow::Result<()>
{
    let (mut ctx, _tmp) = context_with_three_branch_stack()?;
    let subject: gix::refs::FullName = "refs/heads/B".try_into()?;
    let target: gix::refs::FullName = "refs/heads/C".try_into()?;
    let (head_before, subject_tip_before, target_tip_before) = {
        let repo = ctx.repo.get()?;
        (
            repo.head_name()?.expect("HEAD is symbolic").to_owned(),
            repo.rev_parse_single(subject.as_ref())?.detach(),
            repo.rev_parse_single(target.as_ref())?.detach(),
        )
    };
    let order_before = ctx
        .meta()?
        .branch_stack_order(target.as_ref())?
        .expect("branch order is configured");

    let result =
        but_api::branch::move_branch(&mut ctx, subject.as_ref(), target.as_ref(), DryRun::Yes)?;

    assert_workspace_ref(&result.workspace, "refs/heads/B");
    #[cfg(not(feature = "graph-workspace"))]
    assert_eq!(workspace_branch_names(&result.workspace), ["B", "C", "A"]);

    let repo = ctx.repo.get()?;
    assert_eq!(repo.head_name()?.as_ref(), Some(&head_before));
    assert_eq!(repo.rev_parse_single(subject.as_ref())?, subject_tip_before);
    assert_eq!(repo.rev_parse_single(target.as_ref())?, target_tip_before);
    drop(repo);
    assert_eq!(
        ctx.meta()?.branch_stack_order(target.as_ref())?,
        Some(order_before)
    );

    Ok(())
}

#[test]
fn move_checked_out_top_branch_down_checks_out_new_top() -> anyhow::Result<()> {
    let (mut ctx, _tmp) = context_with_three_branch_stack()?;
    let subject: gix::refs::FullName = "refs/heads/C".try_into()?;
    let target: gix::refs::FullName = "refs/heads/A".try_into()?;
    let new_tip: gix::refs::FullName = "refs/heads/B".try_into()?;

    let result =
        but_api::branch::move_branch(&mut ctx, subject.as_ref(), target.as_ref(), DryRun::No)?;

    assert_workspace_ref(&result.workspace, "refs/heads/B");
    #[cfg(not(feature = "graph-workspace"))]
    assert_eq!(
        workspace_branch_names(&result.workspace),
        ["B", "C", "A"],
        "the whole reordered stack should remain projected"
    );
    assert_eq!(
        ctx.repo.get()?.head_name()?.as_ref(),
        Some(&new_tip),
        "HEAD should follow the new top of the reordered stack"
    );
    assert_eq!(
        ctx.meta()?.branch_stack_order(new_tip.as_ref())?,
        Some(vec![new_tip, subject, target]),
        "the persisted branch order should match the graph rewrite"
    );

    Ok(())
}

#[test]
fn move_checked_out_top_branch_down_dry_run_does_not_persist_order() -> anyhow::Result<()> {
    let (mut ctx, _tmp) = context_with_three_branch_stack()?;
    let subject: gix::refs::FullName = "refs/heads/C".try_into()?;
    let target: gix::refs::FullName = "refs/heads/A".try_into()?;
    let order_before = ctx
        .meta()?
        .branch_stack_order(subject.as_ref())?
        .expect("branch order is configured");

    let result =
        but_api::branch::move_branch(&mut ctx, subject.as_ref(), target.as_ref(), DryRun::Yes)?;

    assert_workspace_ref(&result.workspace, "refs/heads/B");
    #[cfg(not(feature = "graph-workspace"))]
    assert_eq!(
        workspace_branch_names(&result.workspace),
        ["B", "C", "A"],
        "dry-run should preview the reordered stack"
    );
    assert_eq!(
        ctx.repo.get()?.head_name()?.as_ref(),
        Some(&subject),
        "dry-run should leave HEAD on the original stack tip"
    );
    assert_eq!(
        ctx.meta()?.branch_stack_order(subject.as_ref())?,
        Some(order_before),
        "dry-run should not persist the proposed order"
    );

    Ok(())
}

#[test]
fn successful_branch_move_returns_and_persists_reordered_stack() -> anyhow::Result<()> {
    let (mut ctx, _tmp) = context_with_three_branch_stack()?;
    let subject: gix::refs::FullName = "refs/heads/A".try_into()?;
    let target: gix::refs::FullName = "refs/heads/B".try_into()?;
    let tip: gix::refs::FullName = "refs/heads/C".try_into()?;

    let result =
        but_api::branch::move_branch(&mut ctx, subject.as_ref(), target.as_ref(), DryRun::No)?;

    assert_workspace_ref(&result.workspace, "refs/heads/C");
    #[cfg(not(feature = "graph-workspace"))]
    assert_eq!(
        workspace_branch_names(&result.workspace),
        ["C", "A", "B"],
        "the returned workspace should use the persisted order"
    );
    assert_eq!(
        ctx.repo.get()?.head_name()?.as_ref(),
        Some(&tip),
        "a lower-branch reorder should leave HEAD on the stack tip"
    );
    assert_eq!(
        ctx.meta()?.branch_stack_order(tip.as_ref())?,
        Some(vec![tip, subject, target]),
        "the successful materialization should persist the new order"
    );

    Ok(())
}

#[test]
fn move_empty_top_branch_below_middle_preserves_commit_ownership() -> anyhow::Result<()> {
    let (mut ctx, _tmp) = context_with_empty_top_branch()?;
    let bottom: gix::refs::FullName = "refs/heads/A".try_into()?;
    let middle: gix::refs::FullName = "refs/heads/B".try_into()?;
    let empty_top: gix::refs::FullName = "refs/heads/C".try_into()?;
    let middle_tip_before = ctx.repo.get()?.rev_parse_single(middle.as_ref())?.detach();
    assert_eq!(
        ctx.repo.get()?.rev_parse_single(empty_top.as_ref())?,
        middle_tip_before,
        "the top branch starts empty"
    );

    // The branch dropzone below B targets A, making the requested order B, C, A (tip to base).
    let result =
        but_api::branch::move_branch(&mut ctx, empty_top.as_ref(), bottom.as_ref(), DryRun::No)?;

    assert_workspace_ref(&result.workspace, "refs/heads/B");
    #[cfg(not(feature = "graph-workspace"))]
    assert_eq!(workspace_branch_names(&result.workspace), ["B", "C", "A"]);

    let repo = ctx.repo.get()?;
    let bottom_tip = repo.rev_parse_single(bottom.as_ref())?.detach();
    assert_eq!(
        repo.rev_parse_single(middle.as_ref())?,
        middle_tip_before,
        "the middle branch keeps owning its commit"
    );
    assert_eq!(
        repo.rev_parse_single(empty_top.as_ref())?,
        bottom_tip,
        "the moved branch stays empty at its new position"
    );
    assert_eq!(
        repo.head_name()?.as_ref(),
        Some(&middle),
        "HEAD follows the new stack tip"
    );
    drop(repo);
    assert_eq!(
        ctx.meta()?.branch_stack_order(middle.as_ref())?,
        Some(vec![middle, empty_top, bottom]),
        "the persisted order matches the ownership-preserving ref move"
    );

    Ok(())
}

#[test]
fn move_empty_branch_dry_run_previews_new_order_without_persisting_it() -> anyhow::Result<()> {
    let (repo, _tmp) = repo_with_feature_branch()?;
    let mut ctx = but_ctx::Context::from_repo_for_testing(repo)?.with_memory_app_cache();
    let main: gix::refs::FullName = "refs/heads/main".try_into()?;
    let middle: gix::refs::FullName = "refs/heads/middle".try_into()?;
    let tip: gix::refs::FullName = "refs/heads/tip".try_into()?;
    create_empty_branch_above(&mut ctx, &middle, &main)?;
    create_empty_branch_above(&mut ctx, &tip, &middle)?;
    let order_before = ctx
        .meta()?
        .branch_stack_order(tip.as_ref())?
        .expect("branch order is configured");

    let result =
        but_api::branch::move_branch(&mut ctx, middle.as_ref(), tip.as_ref(), DryRun::Yes)?;

    assert_workspace_ref(&result.workspace, "refs/heads/middle");
    #[cfg(not(feature = "graph-workspace"))]
    assert_eq!(
        workspace_branch_names(&result.workspace),
        ["middle", "tip", "main", "feature"]
    );
    assert_eq!(
        ctx.repo.get()?.head_name()?.as_ref(),
        Some(&tip),
        "dry-run leaves HEAD on the original tip"
    );
    assert_eq!(
        ctx.meta()?.branch_stack_order(tip.as_ref())?,
        Some(order_before),
        "dry-run leaves the persisted order unchanged"
    );

    Ok(())
}
