# but CLI Instructions

## Command structure and I/O

- Use the `cli-commands` skill for guidance on how to write new CLI commands.
- Some commands define `ERROR_EXAMPLES` in `crates/but/src/args/` that are
  shown on parse errors; keep them accurate when changing a command's
  arguments or behavior.
- Doc comments are read by agents through `but skill reference`, which prints
  each command's first paragraph and its flag help. Say what the command does
  first; when omitting an argument behaves differently in a terminal and in a
  non-interactive run, state both. Flags that only make sense in a terminal
  (a TUI or an editor) take `help_heading = "Interactive"`, which the reference
  omits.

## Debugging Worktree-Lock Deadlocks

When debugging a suspected worktree-lock deadlock, use a debug build with
`BUT_WS_LOCK_DEBUG=1`. In debug builds, this makes worktree guard acquisition
panic when the lock is already held instead of blocking indefinitely. Run the
failing command with a backtrace, for example:

```sh
BUT_WS_LOCK_DEBUG=1 RUST_BACKTRACE=1 cargo run -p but -- -C <repo> <command>
```

Use the panic backtrace to find the nested guard acquisition, then thread the
existing permission to that call site or switch it to an existing
permission-taking helper.

## CLI Tests

- In `crates/but/tests/`, prefer `env.but(...).assert().success()/failure()`
  with `.stdout_eq(snapbox::str![...])` and `.stderr_eq(snapbox::str![...])`.
  For colored terminal output, assert against
  `snapbox::file!["snapshots/<test-name>/<invocation>.stdout.term.svg"]`.
- Use sandbox helpers instead of `std::process::Command::new("git")`:
  `env.invoke_bash(...)` for multi-line command sequences and
  `env.invoke_git("...")` for single Git commands. Do not rewrite existing
  `env.invoke_bash(...)` calls just to use `env.invoke_git(...)`.
- Avoid `env.but(...).output()` followed by direct stdout/stderr assertions;
  keep output checks in snapbox. In tests, use panicking assertions such as
  `assert!`, `assert_eq!`, or `assert_ne!` rather than `anyhow::ensure!`.

## CLI Skills

- After changing CLI commands or workflows, update `crates/but/skill/` so
  bundled agent skills stay current.
