//! `but-<COMMAND>` PATH delegation (Git-style).

use std::{fs, os::unix::fs::PermissionsExt};

use snapbox::str;

use crate::utils::Sandbox;

#[test]
fn delegates_plain_name_to_but_prefixed_executable() {
    let env = Sandbox::empty();
    let bin = env.projects_root().join("external-cmd-bin");
    fs::create_dir_all(&bin).unwrap();
    let helper = bin.join("but-forecast");
    fs::write(
        &helper,
        "#!/bin/sh\nprintf 'args:'\nprintf ' %s' \"$@\"\nprintf '\\n'\n",
    )
    .unwrap();
    let mut perms = fs::metadata(&helper).unwrap().permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&helper, perms).unwrap();

    let path = std::env::var("PATH").unwrap_or_default();
    let new_path = format!("{}:{path}", bin.display());

    env.but("forecast one two")
        .env("PATH", new_path)
        .assert()
        .success()
        .stderr_eq(str![[]])
        .stdout_eq(str![[r#"
args: one two

"#]]);
}

#[test]
fn preserves_external_command_exit_code_without_extra_error() {
    let env = Sandbox::empty();
    let bin = env.projects_root().join("external-cmd-bin");
    fs::create_dir_all(&bin).unwrap();
    let helper = bin.join("but-forecast");
    fs::write(&helper, "#!/bin/sh\nexit 42\n").unwrap();
    let mut perms = fs::metadata(&helper).unwrap().permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&helper, perms).unwrap();

    let path = std::env::var("PATH").unwrap_or_default();
    let new_path = format!("{}:{path}", bin.display());

    // The parent must preserve the helper's status without adding its own diagnostic.
    env.but("forecast")
        .env("PATH", new_path)
        .assert()
        .code(42)
        .stderr_eq(str![[]]);
}

#[test]
fn prefers_builtin_command_when_external_command_clashes() {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack");
    let bin = env.projects_root().join("external-cmd-bin");
    fs::create_dir_all(&bin).unwrap();
    let helper = bin.join("but-alias");
    fs::write(
        &helper,
        "#!/bin/sh\nprintf 'args:'\nprintf ' %s' \"$@\"\nprintf '\\n'\n",
    )
    .unwrap();
    let mut perms = fs::metadata(&helper).unwrap().permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&helper, perms).unwrap();

    let path = std::env::var("PATH").unwrap_or_default();
    let new_path = format!("{}:{path}", bin.display());

    env.but("alias")
        .env("PATH", new_path)
        .assert()
        .success()
        .stderr_eq(str![[]])
        .stdout_eq(str![[r#"
Default aliases (overridable):

  default  →  status
  st       →  status
  stf      →  status --files
  c        →  commit
  s        →  squash
  m        →  move
  wt       →  worktree

"#]]);
}

#[test]
fn propagates_reasonable_error_when_program_is_not_executable() {
    let env = Sandbox::empty();
    let bin = env.projects_root().join("external-cmd-bin");
    fs::create_dir_all(&bin).unwrap();
    let helper = bin.join("but-forecast");
    fs::write(
        &helper,
        "#!/bin/sh\nprintf 'args:'\nprintf ' %s' \"$@\"\nprintf '\\n'\n",
    )
    .unwrap();

    let path = std::env::var("PATH").unwrap_or_default();
    let new_path = format!("{}:{path}", bin.display());

    env.but("forecast one two")
        .env("PATH", new_path)
        .assert()
        .failure()
        .stderr_eq(str![[r#"
Error: could not invoke `"but-forecast"` from PATH

Caused by:
    Permission denied (os error 13)

"#]])
        .stdout_eq(str![[]]);
}

#[test]
fn refuses_to_execute_command_with_forward_slash() {
    let env = Sandbox::empty();

    env.but("bad/command")
            .assert()
            .failure()
            .stderr_eq(str![[r#"
Error: Bad input 'bad/command'

Subcommand contains illegal characters

Hint: Are you trying to write an extension command 'but-<command>'? Make sure that '<command>' only contains characters in the set [a-zA-Z_-]

"#]]);
}

#[test]
fn refuses_to_execute_command_with_dot() {
    let env = Sandbox::empty();

    env.but("bad.command")
            .assert()
            .failure()
            .stderr_eq(str![[r#"
Error: Bad input 'bad.command'

Subcommand contains illegal characters

Hint: Are you trying to write an extension command 'but-<command>'? Make sure that '<command>' only contains characters in the set [a-zA-Z_-]

"#]]);
}

#[test]
fn friendly_error_when_no_such_command_exists() {
    let env = Sandbox::empty();

    env.but("comit").assert().failure().stderr_eq(str![[r#"
error: unrecognized subcommand 'comit'

  tip: some similar subcommands exist: [..]

Usage: but [OPTIONS] [COMMAND]

For more information, try '--help'.

"#]]);
}

#[test]
fn removed_rub_command_teaches_its_replacements() {
    let env = Sandbox::empty();

    env.but("rub ab cd").assert().failure().stderr_eq(str![[r#"

note: `but rub` was retired. Squashing sources into a target is now:

    but squash <source>... -t <target>

Moving sources is `but move <source>...` with a placement flag (`--below`/`--above` a commit, `--branch <branch>`).
See `but squash --help` and `but move --help` for details.
error: unrecognized subcommand 'rub'

Usage: but [OPTIONS] [COMMAND]

For more information, try '--help'.

"#]]);
}
