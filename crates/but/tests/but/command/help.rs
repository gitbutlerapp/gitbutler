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
  -j, --json
          Whether to use JSON output format

      --status-after
          After a mutation command completes, also output workspace status.
          
          In human mode, prints status after the command output. In JSON mode, wraps both in
          {"result": ..., "status": ...} on success, or {"result": ..., "status_error": ...} if the
          status query fails (in which case "status" is absent).

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
  -j, --json          Whether to use JSON output format
      --status-after  After a mutation command completes, also output workspace status
  -h, --help          Print help (see more with '--help')

"#]]);
    Ok(())
}

#[test]
fn nonexistent_path_shows_friendly_error() -> anyhow::Result<()> {
    let env = Sandbox::empty()?;

    env.but("nonexistent-directory-entry")
        .assert()
        .failure()
        .stdout_eq(str![[]])
        .stderr_eq(str![[r#"
Error: "but nonexistent-directory-entry" is not a command. Type "but --help" to see all available commands.

"#]]);

    Ok(())
}
