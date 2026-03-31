use gitbutler_branch_actions::update_workspace_commit;
use gitbutler_operating_modes::{
    INTEGRATION_BRANCH_REF, WORKSPACE_BRANCH_REF, ensure_open_workspace_mode,
};

/// Tests that "verify branch" won't complain if we are on the old integration
/// branch, and that `update_workspace_commit` will put us back on the a branch
/// with the new name.
#[test]
fn works_on_integration_branch() -> anyhow::Result<()> {
    let (ctx, _temp_dir) =
        crate::driverless::writable_context("for-workspace-migration.sh", "workspace-migration")?;

    // Check that we are on the old `gitbutler/integration` branch.
    assert_eq!(
        ctx.git2_repo.get()?.head()?.name(),
        Some(INTEGRATION_BRANCH_REF)
    );

    // Should not throw verification error until migration is complete.
    let guard = ctx.shared_worktree_access();
    let result = ensure_open_workspace_mode(&ctx, guard.read_permission());
    assert!(result.is_ok());

    // Updating workspace commit should put us on the workspace branch.
    update_workspace_commit(&ctx, false)?;
    assert_eq!(
        ctx.git2_repo.get()?.head()?.name(),
        Some(WORKSPACE_BRANCH_REF)
    );
    Ok(())
}

/// Ensures `update_workspace_commit` installs managed hooks on a normal project open (positive path).
///
/// This covers the background hook installation that happens on every workspace commit update —
/// the path the desktop app takes when a project is first added. The fixture repo has no hooks,
/// so all three managed hooks should be freshly created.
#[test]
fn update_workspace_commit_installs_managed_hooks() -> anyhow::Result<()> {
    let (ctx, _temp_dir) =
        crate::driverless::writable_context("for-workspace-migration.sh", "workspace-migration")?;

    let hooks_dir = {
        let repo = ctx.repo.get()?;
        gitbutler_repo::managed_hooks::get_hooks_dir_gix(&repo)
    };

    // Hooks should not exist before the workspace commit is updated.
    for hook_name in ["pre-commit", "post-checkout", "pre-push"] {
        assert!(
            !hooks_dir.join(hook_name).exists(),
            "{hook_name} should not exist before update_workspace_commit"
        );
    }

    update_workspace_commit(&ctx, false)?;

    // All managed hooks must now be installed and carry the GitButler signature.
    for hook_name in ["pre-commit", "post-checkout", "pre-push"] {
        let hook_path = hooks_dir.join(hook_name);
        assert!(
            hook_path.exists(),
            "{hook_name} should be installed after update_workspace_commit"
        );
        let content = std::fs::read_to_string(&hook_path)?;
        assert!(
            content.contains("GITBUTLER_MANAGED_HOOK_V1"),
            "{hook_name} should be GitButler-managed, got: {content}"
        );
    }

    Ok(())
}

/// Ensures the workspace-update path honors the persisted managed-hook installation opt-out.
#[test]
fn update_workspace_commit_skips_hooks_when_disabled_in_git_config() -> anyhow::Result<()> {
    let (ctx, _temp_dir) =
        crate::driverless::writable_context("for-workspace-migration.sh", "workspace-migration")?;

    let repo = ctx.repo.get()?;
    gitbutler_repo::managed_hooks::set_install_managed_hooks_enabled(&repo, false)?;
    let hooks_dir = gitbutler_repo::managed_hooks::get_hooks_dir_gix(&repo);
    drop(repo);

    update_workspace_commit(&ctx, false)?;

    assert!(
        !hooks_dir.join("pre-commit").exists(),
        "pre-commit should not be installed when managed hooks are disabled"
    );
    assert!(
        !hooks_dir.join("post-checkout").exists(),
        "post-checkout should not be installed when managed hooks are disabled"
    );
    Ok(())
}
