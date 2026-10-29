use anyhow::Result;
use but_core::{GitConfigSettings, RepositoryExt as _};
use but_testsupport::visualize_tree;
use snapbox::IntoData;
use tempfile::TempDir;

use but_ctx::Context;

use crate::driverless;

fn command_ctx(name: &str) -> Result<(Context, TempDir)> {
    driverless::writable_context("workspace-commit.sh", name)
}

#[test]
fn conflicting_stacks_do_not_rewrite_workspace_projection() -> Result<()> {
    let (mut ctx, _temp_dir) = command_ctx("conflicting-stacks")?;
    let stack_ids = |ctx: &Context| -> Result<Vec<_>> {
        let guard = ctx.shared_worktree_access();
        Ok(ctx
            .workspace_from_head_uncached(guard.read_permission())?
            .stacks
            .iter()
            .filter_map(|stack| stack.id)
            .collect())
    };
    let before = stack_ids(&ctx)?;
    assert_eq!(before.len(), 2, "both fixture stacks are projected");

    let error = gitbutler_branch_actions::update_workspace_commit(&mut ctx, false).unwrap_err();
    assert!(
        error.to_string().contains("Merge conflict"),
        "conflicting projected stacks are rejected: {error:#}"
    );

    assert_eq!(
        stack_ids(&ctx)?,
        before,
        "workspace commit generation preserves the projected stacks"
    );
    Ok(())
}

#[test]
fn workspace_commits_ignore_commit_signing_configuration() -> Result<()> {
    let (mut ctx, _temp_dir) = command_ctx("adjacent-stacks")?;
    ctx.repo.get()?.set_git_settings(&GitConfigSettings {
        gitbutler_sign_commits: Some(true),
        signing_key: Some("definitely-no-such-signing-key".into()),
        ..Default::default()
    })?;

    let workspace_commit = gitbutler_branch_actions::update_workspace_commit(&mut ctx, false)?;
    let repo = ctx.repo.get()?;
    assert!(
        repo.find_commit(workspace_commit)?
            .decode()?
            .extra_headers()
            .pgp_signature()
            .is_none(),
        "GitButler workspace commits must stay unsigned even when signing is enabled"
    );
    Ok(())
}

#[test]
fn workspace_commit_keeps_unchanged_index_cache_and_sparse_marks() -> Result<()> {
    use gix::index::entry::{Flags, Stage};

    let (mut ctx, _temp_dir) = command_ctx("diverged-stacks")?;
    let previous = {
        let repo = ctx.repo.get()?;
        let mut index = repo.open_index()?;
        let entry = index
            .entry_mut_by_path_and_stage("x".into(), Stage::Unconflicted)
            .expect("the fixture tracks x");
        entry.flags |= Flags::EXTENDED | Flags::SKIP_WORKTREE;
        entry.stat.mtime.secs = 1;
        let previous = entry.stat;
        index.write(Default::default())?;
        previous
    };

    gitbutler_branch_actions::update_workspace_commit(&mut ctx, false)?;

    let index = ctx.repo.get()?.open_index()?;
    assert!(
        index
            .entry_by_path_and_stage("z".into(), Stage::Unconflicted)
            .is_some()
            && index
                .entry_by_path_and_stage("c".into(), Stage::Unconflicted)
                .is_none(),
        "the index tracks the updated workspace tree"
    );
    let entry = index
        .entry_by_path_and_stage("x".into(), Stage::Unconflicted)
        .expect("the workspace still tracks x");
    assert_eq!(
        entry.stat, previous,
        "unchanged files keep their non-racy cache"
    );
    assert!(
        entry.flags.contains(Flags::SKIP_WORKTREE),
        "updating the workspace does not expose an excluded file as deleted"
    );
    Ok(())
}

#[test]
fn deleted_applied_ref_is_not_recreated_from_legacy_metadata() -> Result<()> {
    let (mut ctx, _temp_dir) = command_ctx("adjacent-stacks")?;
    let deleted_head = {
        let repo = ctx.repo.get()?;
        let mut reference = repo.find_reference("refs/heads/stack_b")?;
        let deleted_head = reference.peel_to_commit()?.id;
        reference.delete()?;
        deleted_head
    };

    gitbutler_branch_actions::update_workspace_commit(&mut ctx, false)?;

    let repo = ctx.repo.get()?;
    assert!(
        repo.try_find_reference("refs/heads/stack_b")?.is_none(),
        "projecting the workspace must not recreate a deleted ref"
    );
    let parent_ids = repo
        .find_reference("refs/heads/gitbutler/workspace")?
        .into_fully_peeled_id()?
        .object()?
        .try_into_commit()?
        .parent_ids()
        .map(|id| id.detach())
        .collect::<Vec<_>>();
    assert!(
        !parent_ids.contains(&deleted_head),
        "the deleted ref's stored legacy head must not become a parent"
    );

    Ok(())
}

#[test]
fn update_workspace_commit_installs_no_hooks() -> Result<()> {
    let (mut ctx, _temp_dir) = command_ctx("adjacent-stacks")?;

    gitbutler_branch_actions::update_workspace_commit(&mut ctx, false)?;

    let repo = ctx.repo.get()?;
    for hook in ["pre-commit", "post-checkout"] {
        assert!(
            !repo.git_dir().join("hooks").join(hook).exists(),
            "GitButler no longer installs a {hook} hook into the hooks directory"
        );
    }
    assert!(
        !repo.common_dir().join("gitbutler/hooks").exists(),
        "GitButler no longer writes hook scripts for config-based hooks"
    );
    let config = std::fs::read_to_string(repo.common_dir().join("config"))?;
    assert!(
        !config.contains("gitbutler-pre-commit") && !config.contains("gitbutler-post-checkout"),
        "GitButler no longer registers hooks through git config:\n{config}"
    );
    Ok(())
}

/// Regression test for the same merge-base mismatch in
/// `remerged_workspace_tree_v2`, which updates the workspace commit.
#[test]
fn update_workspace_commit_with_diverged_stacks_preserves_target_content() -> Result<()> {
    let (mut ctx, _temp_dir) = command_ctx("diverged-stacks")?;

    gitbutler_branch_actions::update_workspace_commit(&mut ctx, false)?;

    let repo = ctx.repo.get()?;
    let ws_ref = repo.find_reference("refs/heads/gitbutler/workspace")?;
    let ws_tree_id = ws_ref
        .into_fully_peeled_id()?
        .object()?
        .try_into_commit()?
        .tree_id()?;

    // workspace commit tree should contain x, y, and z when C and D are merged using A as their merge base
    snapbox::assert_data_eq!(
        visualize_tree(ws_tree_id).to_string(),
        snapbox::str![[r#"
8999a87
├── x:100644:587be6b "x\n"
├── y:100644:975fbec "y\n"
└── z:100644:b680253 "z\n"

"#]]
        .raw()
    );

    Ok(())
}
