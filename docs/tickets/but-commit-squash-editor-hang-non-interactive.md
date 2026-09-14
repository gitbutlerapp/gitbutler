Title: `but commit`/`but squash` without `-m` open `$EDITOR` and hang in non-interactive runs, contrary to `--help`

**TLDR:** `CommitMessageSource::from_args` maps "no `-m`, no `--no-message`" to `Editor` unconditionally, with no terminal check, so `but commit`/`but squash` spawn the git editor even when stdin is `/dev/null` and stdout is a pipe. The `--help` text merged in #15853 promises the opposite ("a non-interactive run commits with an empty message" / "skips the editor"). An agent trusting the help hangs its turn; the abandoned run leaves a `but_commit_msg_*.patch` temp file and an orphaned editor process.

## What happened

Expected:

- With no TTY, `but commit -b feat` (no `-m`) commits with an empty message, as `but commit --help` states.

Actual:

- `nvim /var/folders/…/T/but_commit_msg_CZ5RBc.patch` spawned and blocked indefinitely; nothing committed until the editor was killed. ❌
- Same for `but squash <a> -t <b>`; with `EDITOR=true` both proceed with `(no commit message)`.

## Before it broke

Skill dry run, Sep 10 ~15:55 CEST: `but setup` in a scratch repo, one edit, `EDITOR=vim but commit -b feat </dev/null 2>&1 | head`.

## Source

Local dogfooding (kiril) via two independent agent sessions evaluating `crates/but/skill2/SKILL.md`; both hit it.

## Environment

- GitButler: CLI, dev build `f0d45f76311` (2026-09-10)
- System: macOS 25.5.0, `core.editor=nvim`, `EDITOR=vim`, `CURSOR_AGENT` set (agent detection active)
- Repository: fresh single-branch scratch repo, one uncommitted file

## Evidence

- `ps`: `50330  1  01:15 nvim /var/folders/…/T/but_commit_msg_CZ5RBc.patch` (parent already gone after the pipeline was killed).
- `but commit --help`: "Without `-m` or `--no-message`, a terminal opens the editor and a non-interactive run commits with an empty message." (`crates/but/src/args/commit.rs:29-30`)
- `but squash --help`: "a non-interactive run skips the editor" (`crates/but/src/args/squash.rs:30`). Both from `d3fc66f82ed` (PR #15853, merged to master).

## Reproduction

```bash
export E2E_TEST_APP_DATA_DIR=$(mktemp -d)
R=$(mktemp -d); cd $R; git init -q -b main; git config user.email a@b.c; git config user.name t
printf 'a\n' > f.txt; git add .; git commit -qm init; but setup
printf 'a\nb\n' > f.txt
EDITOR=vim but commit -b feat </dev/null 2>&1 | head   # hangs; `ps | grep vim` shows the editor
```

## Diagnosis — Confirmed

1. `CommitMessageSource::from_args(no_message=false, message=None)` returns `Editor { initial: None }` with no environment inspection (`crates/but/src/command/legacy/reword2.rs:401-405`).
2. `execute` → `get_commit_message_from_editor` → `tui::get_text::from_editor` (`crates/but/src/command/legacy/reword.rs:250,382`), which picks `GIT_EDITOR`/`core.editor`/`VISUAL`/`EDITOR` (`crates/but/src/tui/get_text.rs:170-184`) and spawns it with inherited stdio and no TTY check (`get_text.rs:111-117`); with none configured it falls to the built-in TUI editor, which also needs a terminal.
3. `but reword`, `pr new` and `merge` do check and refuse ("A message must be provided via --message (-m) for this output format"); `commit`/`squash` never adopted that path, and #15853 documented the intended behaviour rather than the implemented one.

## Fix direction

Either make `from_args` (or the `Editor` arm of `execute`) refuse or fall back to an empty message when stdin/stdout is not a terminal, matching `reword`'s existing refusal, or correct the two help strings. Pick one so `--help`, `but skill reference` and the skill agree.

## Workaround

Always pass `-m`/`--no-message`; in harnesses set `GIT_EDITOR=true`.

## Impact

Not measured (no PostHog pass). Two of two agent sessions that followed the help text hit it.
