//! `but-<COMMAND>` PATH delegation (Git-style).

use std::{fs, os::unix::fs::PermissionsExt};

use snapbox::str;

use crate::utils::Sandbox;

#[test]
fn delegates_plain_name_to_but_prefixed_executable() -> anyhow::Result<()> {
    let env = Sandbox::empty()?;
    let bin = env.projects_root().join("external-cmd-bin");
    fs::create_dir_all(&bin)?;
    let helper = bin.join("but-forecast");
    fs::write(
        &helper,
        "#!/bin/sh\nprintf 'args:'\nprintf ' %s' \"$@\"\nprintf '\\n'\n",
    )?;
    let mut perms = fs::metadata(&helper)?.permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&helper, perms)?;

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

    Ok(())
}

#[test]
fn prefers_builtin_command_when_external_command_clashes() -> anyhow::Result<()> {
    let env = Sandbox::init_scenario_with_target_and_default_settings("one-stack")?;
    let bin = env.projects_root().join("external-cmd-bin");
    fs::create_dir_all(&bin)?;
    let helper = bin.join("but-alias");
    fs::write(
        &helper,
        "#!/bin/sh\nprintf 'args:'\nprintf ' %s' \"$@\"\nprintf '\\n'\n",
    )?;
    let mut perms = fs::metadata(&helper)?.permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&helper, perms)?;

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

"#]]);

    Ok(())
}

#[test]
fn propagates_reasonable_error_when_program_is_not_executable() -> anyhow::Result<()> {
    let env = Sandbox::empty()?;
    let bin = env.projects_root().join("external-cmd-bin");
    fs::create_dir_all(&bin)?;
    let helper = bin.join("but-forecast");
    fs::write(
        &helper,
        "#!/bin/sh\nprintf 'args:'\nprintf ' %s' \"$@\"\nprintf '\\n'\n",
    )?;

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

    Ok(())
}

#[test]
fn refuses_to_execute_command_with_forward_slash() -> anyhow::Result<()> {
    let env = Sandbox::empty()?;

    env.but("bad/command")
            .assert()
            .failure()
            .stderr_eq(str![[r#"
Error: Bad input 'bad/command'

Subcommand contains illegal characters

Hint: Are you trying to write an extension command 'but-<command>'? Make sure that '<command>' only contains characters in the set [a-zA-Z_-]

"#]]);

    Ok(())
}

#[test]
fn refuses_to_execute_command_with_dot() -> anyhow::Result<()> {
    let env = Sandbox::empty()?;

    env.but("bad.command")
            .assert()
            .failure()
            .stderr_eq(str![[r#"
Error: Bad input 'bad.command'

Subcommand contains illegal characters

Hint: Are you trying to write an extension command 'but-<command>'? Make sure that '<command>' only contains characters in the set [a-zA-Z_-]

"#]]);

    Ok(())
}

#[test]
fn friendly_error_when_no_such_command_exists() -> anyhow::Result<()> {
    let env = Sandbox::empty()?;

    env.but("comit").assert().failure().stderr_eq(str![[r#"
error: unrecognized subcommand 'comit'

  tip: some similar subcommands exist: [..]

Usage: but [OPTIONS] [COMMAND]

For more information, try '--help'.

"#]]);

    Ok(())
}
