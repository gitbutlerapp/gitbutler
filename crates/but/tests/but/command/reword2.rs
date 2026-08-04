use snapbox::str;

use crate::utils::Sandbox;

#[test]
fn rewords_commit_with_multiline_message() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.but("_reword2 tpm -m 'First line\n\n\tSecond paragraph with details'")
        .assert()
        .success()
        .stdout_eq(str![[r#"
Updated commit message for [..]

"#]]);

    env.but("status -vf").assert().success().stdout_eq(str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊● tpm author 2000-01-01 00:00:00 +0000 (sha 3ffa6ce)
┊│     First line  	Second paragraph with details
┊│     tpm:t A A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn rewords_commit_from_editor() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);
    env.file(
        ".git/editor.sh",
        "#!/usr/bin/env bash\nprintf 'Edited in editor\\n' > \"$1\"\n",
    );
    let editor_path = env.projects_root().join(".git/editor.sh");

    env.but("_reword2 tpm")
        .env("GIT_EDITOR", format!("bash {}", editor_path.display()))
        .assert()
        .success();

    env.but("status -vf").assert().success().stdout_eq(str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊● tpm author 2000-01-01 00:00:00 +0000 (sha 4945c18)
┊│     Edited in editor
┊│     tpm:t A A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn formats_commit_message_and_reports_an_unchanged_message() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);
    let long_message = "Subject\n\nThis is a long commit message body that should be wrapped because it is wider than seventy two characters in total";

    env.but(format!("_reword2 tpm -m '{long_message}'"))
        .assert()
        .success();
    env.but("_reword2 tpm --fix-formatting").assert().success();

    snapbox::assert_data_eq!(
        env.invoke_git("show -s --format=%B refs/heads/A"),
        str![[r#"
Subject

This is a long commit message body that should be wrapped because it is
wider than seventy two characters in total
"#]]
    );

    env.but("_reword2 tpm --fix-formatting")
        .assert()
        .success()
        .stdout_eq(str![[r#"
No changes to commit message

"#]]);

    env.but("_reword2 tpm --fix-formatting --json")
        .assert()
        .success()
        .stdout_eq(str![[r#"
{
  "type": "commitUnchanged",
  "changed": false,
  "commitId": "d19a16a700b2eb611f0ef4235413b1cadf53fd1e",
  "changeId": "tpmktkqkknswxzyszlkxlrzoqorvpmur"
}

"#]]);
}

#[test]
fn emits_json_for_commit_reword() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.but("_reword2 tpm -m 'Updated commit message' --json")
        .assert()
        .success()
        .stdout_eq(str![[r#"
{
  "type": "commitUpdated",
  "changed": true,
  "sourceCommitId": "9477ae721ab521d9d0174f70e804ce3ff9f6fb56",
  "sourceChangeId": "tpmktkqkknswxzyszlkxlrzoqorvpmur",
  "newCommitId": "dfe058b3cb3a8d729e6e1fe1496c13ff544cc543",
  "newChangeId": "tpmktkqkknswxzyszlkxlrzoqorvpmur"
}

"#]]);

    env.but("status -vf").assert().success().stdout_eq(str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [A]
┊● tpm author 2000-01-01 00:00:00 +0000 (sha dfe058b)
┊│     Updated commit message
┊│     tpm:t A A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);
}

#[test]
fn protects_merged_upstream_commits_unless_allowed() {
    let env =
        Sandbox::init_scenario_with_target_and_default_settings("upstream-integrated-with-updates");
    env.setup_metadata_at_target(&["A", "B"], "refs/heads/base");

    env.but("_reword2 A -m renamed")
        .env("NO_BG_TASKS", "1")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Error: Branch 'A' is merged upstream

Hint: Most likely you want `but pull`, which updates the workspace and removes landed work. In rare cases `--allow-merged` can bypass this check

"#]]);

    env.but("_reword2 nyq -m 'new message'")
        .env("NO_BG_TASKS", "1")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Error: Commit 756ee31 is merged upstream

Hint: Most likely you want `but pull`, which updates the workspace and removes landed work. In rare cases `--allow-merged` can bypass this check

"#]]);

    env.but("_reword2 nyq -m 'new message' --allow-merged")
        .env("NO_BG_TASKS", "1")
        .assert()
        .success();

    env.but("status -vf")
        .assert()
        .success()
        .stdout_eq(str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ g0 [A] (merged upstream)
┊● nyq author 2000-01-01 00:00:00 +0000 (sha b019c15)
┊│     new message
┊│     nyq:z A file-a.txt
├╯
┊
┊╭┄ h0 [B]
┊● kyl author 2000-01-01 00:00:00 +0000 (sha 536958e)
┊│     B-change 
┊│     kyl:l A file-b.txt
├╯
┊
┊● 9354ac4 (upstream: origin/main) 2 new commits
├╯ efc9211 (common base) 2000-01-02 base

Hint: origin/main moved ahead; run `but pull` to update the workspace
Hint: branches marked `(merged upstream)` have landed; run `but pull` to remove them, or start new work on another branch

"#]]);
}

#[test]
fn renames_and_validates_branches() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);

    env.but("_reword2 A -m renamed-branch")
        .assert()
        .success()
        .stdout_eq(str![[r#"
Renamed 'A' to 'renamed-branch'

"#]]);

    env.but("status -vf").assert().success().stdout_eq(str![[r#"
╭┄ zz [uncommitted] (no changes)
┊
┊╭┄ re [renamed-branch]
┊● tpm author 2000-01-01 00:00:00 +0000 (sha 9477ae7)
┊│     add A 
┊│     tpm:t A A
├╯
┊
┴ 0dc3733 (common base) 2000-01-02 add M

Hint: run `but help` for all commands

"#]]);

    env.but("_reword2 renamed-branch -m HEAD")
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Error: Bad input 'HEAD'

Invalid branch name: Could not turn "HEAD" into a valid reference name

"#]]);
}

#[test]
fn renames_the_checked_out_branch_in_single_branch_mode() {
    let env = Sandbox::open_with_default_settings("one-fork");

    env.but("_reword2 main -m renamed")
        .assert()
        .success()
        .stdout_eq(str![[r#"
Renamed 'main' to 'renamed'

"#]]);

    assert_eq!(
        env.invoke_git("symbolic-ref HEAD"),
        "refs/heads/renamed",
        "HEAD follows the renamed branch"
    );
    let repo = env.open_repo();
    assert!(
        repo.try_find_reference("refs/heads/main")
            .unwrap()
            .is_none(),
        "the old branch reference was removed"
    );
    assert!(
        repo.try_find_reference("refs/heads/renamed")
            .unwrap()
            .is_some(),
        "the renamed branch reference was created"
    );
}
