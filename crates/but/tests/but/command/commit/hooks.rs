use snapbox::str;

use crate::utils::Sandbox;

fn install_hook(env: &Sandbox, name: &str, script: &str) {
    env.file(
        format!(".git/hooks/{name}"),
        format!("#!/bin/sh\n{script}\n"),
    );
    env.invoke_bash(format!("chmod +x .git/hooks/{name}"));
}

/// Hooks that log when they run, with pre-commit also logging the staged paths. commit-msg adds
/// a trailer and post-commit fails.
fn install_logging_hooks(env: &Sandbox) {
    install_hook(
        env,
        "pre-commit",
        "{ echo pre-commit; git diff --cached --name-only; } >> .git/hook.log",
    );
    install_hook(
        env,
        "commit-msg",
        "echo commit-msg >> .git/hook.log\nprintf '\\nSigned-off-by: Hook <hook@example.com>\\n' >> \"$1\"",
    );
    install_hook(
        env,
        "post-commit",
        "echo post-commit >> .git/hook.log\necho 'could not notify' >&2\nexit 1",
    );
}

fn hook_log(env: &Sandbox) -> String {
    std::fs::read_to_string(env.projects_root().join(".git/hook.log")).unwrap()
}

#[test]
fn runs_hooks_around_the_commit() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);
    install_logging_hooks(&env);
    env.file("editor.sh", "printf 'commit from editor\\n' > \"$1\"\n");
    let editor_command = format!("sh {}", env.projects_root().join("editor.sh").display());
    env.file("file.txt", "Some text");
    env.file("other.txt", "Not committed");

    // The commit is already made when post-commit runs, so its failure is only a warning.
    env.but("commit file.txt")
        .env("GIT_EDITOR", editor_command)
        .assert()
        .success()
        .stdout_eq(str![[r#"
Created commit [..] on branch 'A'
Warning: post-commit hook failed:
could not notify

"#]]);

    assert_eq!(
        hook_log(&env),
        "pre-commit\nfile.txt\ncommit-msg\npost-commit\n",
        "each hook runs once, in Git's order, and pre-commit sees only the selected change staged"
    );
    // commit-msg sees the message from the editor, and its edit is what gets committed.
    snapbox::assert_data_eq!(
        env.invoke_git("log -1 --format=%B refs/heads/A"),
        str![[r#"
commit from editor

Signed-off-by: Hook <hook@example.com>
"#]]
    );
}

#[test]
fn runs_hooks_around_the_first_commit() {
    let env = Sandbox::open_with_default_settings("unborn");
    install_logging_hooks(&env);

    env.but("commit -m 'initial commit'").assert().success();

    assert_eq!(
        hook_log(&env),
        "pre-commit\ncommit-msg\npost-commit\n",
        "an unborn repository runs the same hooks"
    );
    snapbox::assert_data_eq!(
        env.invoke_git("log -1 --format=%B"),
        str![[r#"
initial commit

Signed-off-by: Hook <hook@example.com>
"#]]
    );
}

#[test]
fn failing_hook_aborts_the_commit_unless_hooks_are_skipped() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);
    env.file("file.txt", "Some text");
    let tip = env.invoke_git("rev-parse refs/heads/A");

    for hook in ["pre-commit", "commit-msg"] {
        install_hook(&env, hook, "echo 'lint failed' >&2\nexit 1");
        env.but("commit -m 'add file.txt'")
            .assert()
            .failure()
            .stdout_eq(str![])
            .stderr_eq(format!("Error: {hook} hook failed:\nlint failed\n"));
        assert_eq!(
            env.invoke_git("rev-parse refs/heads/A"),
            tip,
            "a failing {hook} hook leaves no commit behind"
        );
        env.invoke_bash(format!("rm .git/hooks/{hook}"));
    }

    install_hook(&env, "pre-commit", "exit 1");
    env.but("commit --no-hooks -m 'add file.txt'")
        .assert()
        .success();
    assert_ne!(
        env.invoke_git("rev-parse refs/heads/A"),
        tip,
        "--no-hooks commits despite the failing hook"
    );
}

#[test]
fn commits_files_as_a_formatting_pre_commit_hook_left_them() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    env.setup_metadata(&["A"]);
    env.file("file.txt", "one\n");
    env.but("commit -m 'add file.txt'").assert().success();
    env.file("file.txt", "one\ntwo\n");
    install_hook(&env, "pre-commit", "printf 'formatted\\n' >> file.txt");

    // The hook's rewrite moves the hunk that was selected before it ran.
    env.but("commit -m 'add two'").assert().success();

    snapbox::assert_data_eq!(
        env.invoke_git("show refs/heads/A:file.txt"),
        str![[r#"
one
two
formatted
"#]]
    );
}
