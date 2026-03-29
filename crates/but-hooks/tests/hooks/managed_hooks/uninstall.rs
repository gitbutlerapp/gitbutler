use anyhow::Result;
use but_hooks::managed_hooks::uninstall_managed_hooks;

use super::{
    create_hooks_dir, create_hooks_dir_with_managed_hooks, create_managed_hook, create_user_hook,
    hook_exists, read_hook,
};

#[test]
fn removes_managed_hooks() -> Result<()> {
    let (_temp, hooks_dir) = create_hooks_dir_with_managed_hooks()?;

    uninstall_managed_hooks(&hooks_dir)?;

    super::assert_no_hooks_exist(&hooks_dir);
    Ok(())
}

#[test]
fn restores_user_hooks() -> Result<()> {
    let (_temp, hooks_dir) = create_hooks_dir()?;

    let user_hook_content = "#!/bin/sh\n# User's custom hook\necho 'user hook'\n";

    // Simulate prior GitButler installation: managed hook + user backup
    create_managed_hook(&hooks_dir, "pre-commit")?;
    create_user_hook(&hooks_dir, "pre-commit-user", user_hook_content)?;

    // Uninstall should restore the backup
    uninstall_managed_hooks(&hooks_dir)?;

    assert!(
        hook_exists(&hooks_dir, "pre-commit"),
        "User hook should be restored"
    );
    assert!(
        !hook_exists(&hooks_dir, "pre-commit-user"),
        "Backup should be removed after restore"
    );

    let restored_content = read_hook(&hooks_dir, "pre-commit")?;
    assert_eq!(
        restored_content, user_hook_content,
        "Restored hook should have original content"
    );
    Ok(())
}

#[test]
fn does_not_remove_non_managed_hooks() -> Result<()> {
    let (_temp, hooks_dir) = create_hooks_dir()?;

    // Create a non-GitButler hook
    let user_hook = "#!/bin/sh\n# Not a GitButler hook\necho 'user hook'\n";
    create_user_hook(&hooks_dir, "pre-commit", user_hook)?;

    // Try to uninstall - should not remove the hook
    let result = uninstall_managed_hooks(&hooks_dir)?;

    // Hook should still exist
    assert!(
        hook_exists(&hooks_dir, "pre-commit"),
        "Non-managed hook should not be removed"
    );
    let content = read_hook(&hooks_dir, "pre-commit")?;
    assert_eq!(content, user_hook, "Hook content should be unchanged");

    // Summary should have no removed/restored hooks (user hook was left untouched)
    assert!(
        result.removed.is_empty() && result.restored.is_empty(),
        "non-managed hook should not appear in removed or restored: {result:?}"
    );
    Ok(())
}

#[test]
fn summary_reports_removed_and_restored_separately() -> Result<()> {
    let (_temp, hooks_dir) = create_hooks_dir()?;

    // Install all three GB hooks, with a user backup for pre-commit only.
    let user_pre_commit = "#!/bin/sh\necho 'my pre-commit'\n";
    create_managed_hook(&hooks_dir, "pre-commit")?;
    create_managed_hook(&hooks_dir, "post-checkout")?;
    create_managed_hook(&hooks_dir, "pre-push")?;
    create_user_hook(&hooks_dir, "pre-commit-user", user_pre_commit)?;

    let summary = uninstall_managed_hooks(&hooks_dir)?;

    // pre-commit had a backup → restored; the other two → removed.
    assert_eq!(summary.restored, vec!["pre-commit"]);
    assert!(
        summary.removed.contains(&"post-checkout".to_owned()),
        "post-checkout should be in removed"
    );
    assert!(
        summary.removed.contains(&"pre-push".to_owned()),
        "pre-push should be in removed"
    );
    assert_eq!(summary.removed.len(), 2);
    assert!(summary.warnings.is_empty(), "no warnings expected");
    Ok(())
}

#[test]
fn is_idempotent() -> Result<()> {
    let (_temp, hooks_dir) = create_hooks_dir_with_managed_hooks()?;

    // Uninstall twice
    let result1 = uninstall_managed_hooks(&hooks_dir)?;
    let result2 = uninstall_managed_hooks(&hooks_dir)?;

    // First uninstall: all three GB hooks removed.
    assert_eq!(
        result1.removed.len(),
        3,
        "first uninstall should remove 3 hooks"
    );
    assert!(result1.restored.is_empty());
    // Second uninstall: nothing to do (hooks already gone).
    assert!(
        result2.removed.is_empty() && result2.restored.is_empty() && result2.warnings.is_empty(),
        "second uninstall should be a no-op: {result2:?}"
    );
    Ok(())
}
