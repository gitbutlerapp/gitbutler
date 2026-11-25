use crate::utils::Sandbox;

#[test]
fn json_flag_goes_after_the_subcommand() {
    // TODO: use an actual repository here, but single-branch mode isn't really supported yet
    //       so everything fails anyway.
    let env = Sandbox::empty();

    use snapbox::str;

    use crate::utils::CommandExt;
    env.but("status --json")
        .allow_json()
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Error: No git repository found at .git

Hint: Run `but init` to initialize a Git repository.

"#]]);

    // The retired position before the subcommand still works, with a note.
    env.but("--json status")
        .allow_json()
        .assert()
        .failure()
        .stderr_eq(str![[r#"

note: `--json` now goes after the subcommand, as in `but status --json`. Placing it before the subcommand will stop working in a future release.
Error: No git repository found at .git

Hint: Run `but init` to initialize a Git repository.

"#]]);
}

#[test]
fn json_flag_is_rejected_by_commands_that_dont_print_json() {
    let env = Sandbox::empty();

    use snapbox::str;

    use crate::utils::CommandExt;
    env.but("completions zsh --json")
        .allow_json()
        .assert()
        .failure()
        .stdout_eq(str![])
        .stderr_eq(str![[r#"
error: unexpected argument '--json' found
...
"#]]);

    env.but("--json completions zsh")
        .allow_json()
        .assert()
        .failure()
        .stdout_eq(str![])
        .stderr_eq(str![[r#"
error: `but completions` doesn't print JSON, so it doesn't take `--json`
...
"#]]);
}

#[test]
fn json_from_the_environment_does_not_apply_to_commands_that_dont_print_json() {
    let env = Sandbox::empty();

    env.but("help")
        .env("BUT_OUTPUT_FORMAT", "json")
        .assert()
        .success()
        .stdout_eq(snapbox::str![[r#"
The GitButler CLI change control system
...
"#]]);
}

#[test]
fn json_before_an_unknown_command_reports_the_unknown_command() {
    let env = Sandbox::empty();

    env.but("--json stauts")
        .assert()
        .failure()
        .stdout_eq(snapbox::str![])
        .stderr_eq(snapbox::str![[r#"
error: unrecognized subcommand 'stauts'
...
"#]]);
}

#[test]
fn default_command_respects_c_flag_for_setup_checks() {
    let env = Sandbox::empty();
    env.invoke_bash("git init repo");

    env.but("-C repo")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┴ no commits (main)

Hint: run `but commit` to create the initial commit

"#]]);
}

#[test]
fn default_alias_can_provide_c_flag_for_implicit_default_command() {
    let env = Sandbox::empty();
    env.invoke_bash(
        r#"
git init .
git init repo
git config but.alias.default "-C repo status"
"#,
    );

    env.but("")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┴ no commits (main)

Hint: run `but commit` to create the initial commit

"#]]);
}

#[test]
fn explicit_c_flag_overrides_default_alias_c_flag() {
    let env = Sandbox::empty();
    env.invoke_bash(
        r#"
git init .
git init repo
git init repo2
git config but.alias.default "-C repo status"
"#,
    );

    env.but("-C repo2")
        .assert()
        .success()
        .stderr_eq(snapbox::str![])
        .stdout_eq(snapbox::str![[r#"
╭┄ @ [uncommitted] (no changes)
┊
┴ no commits (main)

Hint: run `but commit` to create the initial commit

"#]]);
}
