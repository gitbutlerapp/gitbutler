use bstr::ByteSlice;
use snapbox::str;

use crate::utils::Sandbox;

#[cfg(feature = "legacy")]
#[test]
fn rub_looks_good() -> anyhow::Result<()> {
    let env = Sandbox::empty()?;

    // Assert on plain help text so the test doesn't drift on ANSI-to-SVG styling differences.
    env.but("rub --help").assert().success().stdout_eq(str![[r#"
Combines two entities together to perform an operation like amend, squash, stage, or move.

The `rub` command is a simple verb that helps you do a number of editing
operations by doing combinations of two things.

For example, you can "rub" a file onto a branch to stage that file to
the branch. You can also "rub" a commit onto another commit to squash
them together. You can rub a commit onto a branch to move that commit.
You can rub a file from one commit to another.

## Operations Matrix

Each cell shows what happens when you rub SOURCE → TARGET:

```text
SOURCE ↓ / TARGET →  │ zz (unassigned) │ Commit     │ Branch      │ Stack
─────────────────────┼─────────────────┼────────────┼─────────────┼────────────
File/Hunk            │ Unstage         │ Amend      │ Stage       │ Stage
Commit               │ Undo            │ Squash     │ Move        │ -
Branch (all changes) │ Unstage all     │ Amend all  │ Reassign    │ Reassign
Stack (all changes)  │ Unstage all     │ -          │ Reassign    │ Reassign
Unassigned (zz)      │ -               │ Amend all  │ Stage all   │ Stage all
File-in-Commit       │ Uncommit        │ Move       │ Uncommit to │ -
```

Legend:
- `zz` is a special target meaning "unassigned" (no branch)
- `-` means the operation is not supported
- "all changes" / "all" refers to all uncommitted changes from that source

## Examples

Squashing two commits into one (combining the commit messages):

```text
but rub 3868155 abe3f53f
```

Amending a commit with the contents of a modified file:

```text
but rub README.md abe3f53f
```

Moving a commit from one branch to another:

```text
but rub 3868155 feature-branch
```

Usage: but rub [OPTIONS] <SOURCE> <TARGET>

Arguments:
  <SOURCE>
          The source entity to combine

  <TARGET>
          The target entity to combine with the source

Options:
      --format <FORMAT>
          Explicitly control how output should be formatted.
          
          If unset and from a terminal, it defaults to human output, when redirected it's for
          shells.

          Possible values:
          - human: The output to write is supposed to be for human consumption, and can be more
            verbose
          - shell: The output should be suitable for shells, and assigning the major result to
            variables so that it can be reused in subsequent CLI invocations
          - json:  Output detailed information as JSON for tool consumption
          - none:  Do not output anything, like redirecting to /dev/null
          
          [env: BUT_OUTPUT_FORMAT=]
          [default: human]

  -h, --help
          Print help (see a summary with '-h')

"#]]);
    env.but("rub -h").assert().success().stdout_eq(str![[r#"
Combines two entities together to perform an operation like amend, squash, stage, or move.

Usage: but rub [OPTIONS] <SOURCE> <TARGET>

Arguments:
  <SOURCE>  The source entity to combine
  <TARGET>  The target entity to combine with the source

Options:
      --format <FORMAT>  Explicitly control how output should be formatted [env: BUT_OUTPUT_FORMAT=]
                         [default: human] [possible values: human, shell, json, none]
  -h, --help             Print help (see more with '--help')

"#]]);
    Ok(())
}

#[test]
fn nonexistent_comman_shows_friendly_error() -> anyhow::Result<()> {
    let env = Sandbox::empty()?;

    env.but("no-such-command")
        .assert()
        .failure()
        .stdout_eq(str![[]])
        .stderr_eq(str![[r#"
error: unrecognized subcommand 'no-such-command'

Usage: but [OPTIONS] [COMMAND]

For more information, try '--help'.

"#]]);

    Ok(())
}

#[test]
/// We want the output of `help --help` to be the same as `help`.
fn help_help_should_be_help() -> anyhow::Result<()> {
    let env = Sandbox::empty()?;

    let help = env.but("help").output()?.stdout;
    env.but("help --help")
        .assert()
        .success()
        .stdout_eq(help.to_str_lossy().to_string());

    Ok(())
}
