# but CLI Instructions

These supplement `crates/AGENTS.md` for work under `crates/but/`.

For CLI work that touches graph/workspace/branch/stack/commit relationships,
reachability, ordering, operation targets, or Git graph/history/ref-placement
changes, also read `crates/WORKSPACE_MODEL.md`.

## Command structure and I/O

- Use the `cli-commands` skill for guidance on how to write new CLI commands.
- Some commands define `ERROR_EXAMPLES` in `crates/but/src/args/` that are
  shown on parse errors; keep them accurate when changing a command's
  arguments or behavior.

## Worktree Guards And Deadlocks

Command handlers should acquire the required worktree guard at the top of the
operation and pass the derived permission down the call chain. Prefer
permission-taking helpers such as `*_with_perm(...)` when a guard is already
held. Do not call helpers that acquire another shared or exclusive worktree
guard while the command is still holding one.

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
  with `.stdout_eq(snapbox::str![...])` and
  `.stderr_eq(snapbox::str![...])`; use `[..]` or `...` wildcards for unstable
  portions instead of weakening the assertion.
- Update CLI snapshots with `SNAPSHOTS=overwrite cargo test -p but`,
  scoped to a test name when possible. For colored terminal output, assert
  against `snapbox::file!["snapshots/<test-name>/<invocation>.stdout.term.svg"]`
  and update with the same command.
    - When updating snapshots, always inspect the results to ensure that the
      tests still test what they claim to
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
