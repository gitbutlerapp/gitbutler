use anyhow::Context as _;
use snapbox::str;

use super::util::enter_edit_mode_with_conflicted_commit;
use crate::utils::Sandbox;

fn current_branch_name(env: &Sandbox) -> anyhow::Result<String> {
    let repo = env.open_repo()?;
    repo.rev_parse_single("HEAD")
        .context("HEAD should resolve")?;
    repo.head_name()?
        .map(|name| name.as_ref().shorten().to_string())
        .context("HEAD should point to a branch")
}

#[test]
fn resolve_status_and_finish_work_in_edit_mode() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    enter_edit_mode_with_conflicted_commit(&env)?;

    env.but("resolve status")
        .assert()
        .success()
        .stderr_eq(str![""]);

    env.file("test-file.txt", "resolved content\n");
    env.invoke_git("add test-file.txt");

    env.but("resolve finish")
        .assert()
        .success()
        .stderr_eq(str![""]);

    assert_eq!(current_branch_name(&env)?, "gitbutler/workspace");
    Ok(())
}

#[test]
fn resolve_cancel_works_in_edit_mode() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    enter_edit_mode_with_conflicted_commit(&env)?;

    env.but("resolve cancel --force")
        .assert()
        .stderr_eq(str![""])
        .success();
    assert_eq!(current_branch_name(&env)?, "gitbutler/workspace");
    Ok(())
}

#[test]
fn resolve_cancel_requires_force_when_changes_were_made() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    enter_edit_mode_with_conflicted_commit(&env)?;

    env.file("test-file.txt", "resolved content with additional edits\n");

    env.but("resolve cancel")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Failed to handle conflict resolution. There are changes that differ from the original commit you were editing. Canceling will drop those changes.

If you want to go through with this, please re-run with `--force`.

If you want to keep the changes you have made, consider finishing the resolution and then moving the changes with the rub command.

"#]]);

    env.but("resolve cancel --force")
        .assert()
        .success()
        .stderr_eq(str![""]);

    assert_eq!(current_branch_name(&env)?, "gitbutler/workspace");
    Ok(())
}
