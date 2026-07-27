use bstr::ByteSlice;
use snapbox::str;

use crate::utils::Sandbox;

#[test]
fn nonexistent_comman_shows_friendly_error() {
    let env = Sandbox::empty();

    env.but("no-such-command")
        .assert()
        .failure()
        .stdout_eq(str![[]])
        .stderr_eq(str![[r#"
error: unrecognized subcommand 'no-such-command'

Usage: but [OPTIONS] [COMMAND]

For more information, try '--help'.

"#]]);
}

#[test]
/// We want the output of `help --help` to be the same as `help`.
fn help_help_should_be_help() -> anyhow::Result<()> {
    let env = Sandbox::empty();

    let help = env.but("help").output()?.stdout;
    env.but("help --help")
        .assert()
        .success()
        .stdout_eq(help.to_str_lossy().to_string());

    Ok(())
}

#[test]
fn top_level_help_honors_agent_format_after_help_flag() -> anyhow::Result<()> {
    let env = Sandbox::empty();
    let help = env
        .but("help")
        .env("PI_CODING_AGENT", "true")
        .output()?
        .stdout;

    env.but("--help")
        .env("PI_CODING_AGENT", "true")
        .assert()
        .success()
        .stdout_eq(help.to_str_lossy().to_string());

    env.but("--help")
        .env("PI_CODING_AGENT", "true")
        .assert()
        .success()
        .stdout_eq(help.to_str_lossy().to_string());

    Ok(())
}
