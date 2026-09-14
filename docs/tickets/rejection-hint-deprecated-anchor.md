Title: Dependency-rejection hint still prints the deprecated `but branch new … --anchor`

**TLDR:** When `but commit -b <new-branch>` is rejected because a hunk depends on another branch, `rejection.rs` suggests `but branch new <name> --anchor <dep>`. `--anchor` was hidden and deprecated in favour of `--above` when `branch new` was re-implemented (`c407890d8fd`, Aug 7); following the hint now prints a deprecation warning, the hint contradicts `but branch new --help`, and the skill/reference cannot document it because the flag is hidden.

## What happened

Expected:

- Hint reads `but branch new feat-b --above feat-a`.

Actual:

- `Hint: to apply these changes, create feat-b stacked on top of feat-a and try again:` / `but branch new feat-b --anchor feat-a` ❌
- Running it prints `⚠ --anchor/-a is deprecated and will be removed in a future release. Use --above/-A instead`.

## Before it broke

Skill dry run, Sep 10 ~15:40 CEST: `but commit -b feat-a … <hunk>` then `but commit -b feat-b -m "everything else"` where the remaining hunk depends on `feat-a`.

## Source

Local dogfooding (kiril), agent dry run; also seen in a donated production trace (usage research casebook C11) where the agent hit the warning.

## Environment

- GitButler: CLI, dev build `f0d45f76311` (2026-09-10)
- System: macOS 25.5.0
- Repository: scratch repo, one applied branch `feat-a`, one uncommitted hunk overlapping its commit

## Evidence

- `crates/but/src/utils/rejection.rs:289-299`: `(None, Target::NewBranch(Some(name))) => … format!("but branch new {} --anchor {}", …)` (last touched `d527d57c2d55`, Jul 28).
- `crates/but/src/args/branch.rs:179-182`: `anchor` has `hide = true`.
- `crates/but/src/command/legacy/branch/new.rs:87`: deprecation warning text.
- The sibling arm for an existing branch (`rejection.rs:276-288`) already suggests `but move … --above …`.

## Reproduction

```bash
export E2E_TEST_APP_DATA_DIR=$(mktemp -d)
R=$(mktemp -d); cd $R; git init -q -b main; git config user.email a@b.c; git config user.name t
printf 'l1\nl2\n' > a.txt; git add .; git commit -qm init; but setup
printf 'l1\nl2\nl3\n' > a.txt
but commit -b feat-a -m "add l3"                 # commits the hunk
printf 'l1\nl2\nl3\nl4\n' > a.txt
but commit -b feat-b -m "add l4"                 # rejected: depends on feat-a; hint shows --anchor
```

## Diagnosis — Confirmed

The hint string at `rejection.rs:296` was written before `--anchor` was deprecated and was not updated in `c407890d8fd`.

## Fix direction

Change the format string to `--above`; add a snapshot test on the rejection hint so a future flag rename fails CI.

## Workaround

Run `but branch new <name> --above <dep>` (or `but move <existing> --above <dep>`) instead of the printed command.

## Impact

Not measured. Low severity (warning only), but the hint is the one recovery path agents follow literally.
