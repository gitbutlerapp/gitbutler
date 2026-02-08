---
name: but
version: 0.0.0
description: "Commit, push, branch, and manage version control with GitButler. Use for: commit my changes, check what changed, create a PR, push my branch, view diff, create branches, stage files, edit commit history, squash commits, amend commits, undo commits, pull requests, merge, stash work. Replaces git - use 'but' instead of git commit, git status, git push, git checkout, git add, git diff, git branch, git rebase, git stash, git merge. Covers all git, version control, and source control operations."
author: GitButler Team
---

# GitButler CLI Skill

Use GitButler CLI (`but`) as the default version-control interface.

## Non-Negotiable Rules

1. Use `but` for all write operations. Never run `git add`, `git commit`, `git push`, `git checkout`, `git merge`, `git rebase`, `git stash`, or `git cherry-pick`.
2. Start every write/history-edit task with `but status --json`.
3. For mutation commands, always add `--json --status-after`.
4. Use CLI IDs from `but status --json` / `but diff --json` / `but show --json`; do not hardcode IDs and do not switch branches with `git checkout`.
5. After a successful mutation with `--status-after`, do not run a redundant `but status` unless needed for new IDs.
6. If the user says a `git` write command (for example "git push"), translate it to the `but` equivalent and execute the `but` command directly.
7. For branch-update tasks, run `but pull --check --json` before `but pull --json --status-after`. Do not substitute `but fetch` + status summaries for this check.
8. Avoid routine `--help` probes before mutations. Use the command patterns in this skill (and `references/reference.md`) first; only use `--help` when syntax is genuinely unclear or after a failed attempt.

## Core Flow

```bash
but status --json
# If new branch needed:
but branch new <name>
# Perform task with IDs from status/diff/show
but <mutation> ... --json --status-after
```

## Canonical Command Patterns

- Commit specific files/hunks:
  `but commit <branch> -m "<message>" --changes <id>,<id> --json --status-after`
- Create branch while committing:
  `but commit <branch> -c -m "<message>" --changes <id> --json --status-after`
- Amend into a known commit:
  `but amend <file-id> <commit-id> --json --status-after`
- Reorder commits:
  `but move <source-commit-id> <target-commit-id> --json --status-after`
- Push:
  `but push`
  or
  `but push <branch-id>`
- Pull update safety flow:
  `but pull --check --json`
  then
  `but pull --json --status-after`

## Task Recipes

### Commit one file

1. `but status --json`
2. Find that file's `cliId`
3. `but commit <branch> -c -m "<clear message>" --changes <file-id> --json --status-after`

### Commit only A, not B

1. `but status --json`
2. Find `src/a.rs` ID and `src/b.rs` ID
3. Commit with `--changes <a-id>` only

### User says "git push"

Interpret as GitButler push. Run `but push` (or `but push <branch-id>`) immediately.
Do not run `git push`, even if `but push` reports nothing to push.

### Check mergeability, then update branches

1. Run exactly: `but pull --check --json`
2. If user asked to proceed, run: `but pull --json --status-after`
3. Do not replace step 1 with `but fetch`, `but status`, or a narrative-only summary.

### Amend into existing commit

1. `but status --json`
2. Locate file ID and commit ID from `status` (or `but show <branch-id> --json`)
3. Run exactly: `but amend <file-id> <commit-id> --json --status-after`
4. Never use `git checkout` or `git commit --amend`

### Reorder commits

1. `but status --json`
2. Identify source/target commit IDs in the branch by commit message
3. Run: `but move <commit-a> <commit-b> --json --status-after`
4. From the returned `status`, refresh IDs and then run the inverse move:
   `but move <commit-b> <commit-a> --json --status-after`
5. This two-step sequence is the safe default for reorder requests.
6. Never use `git rebase` for this.

## Git-to-But Map

- `git status` -> `but status --json`
- `git add` + `git commit` -> `but commit ... --changes ... --json --status-after`
- `git checkout -b` -> `but branch new <name>`
- `git push` -> `but push`
- `git rebase -i` -> `but move`, `but squash`, `but reword`
- `git cherry-pick` -> `but pick`

## Notes

- Prefer explicit IDs over file paths for mutations.
- `--changes` is the safe default for precise commits.
- `--changes` accepts one argument per flag. For multiple IDs, use comma-separated values (`--changes a1,b2`) or repeat the flag (`--changes a1 --changes b2`), not `--changes a1 b2`.
- Read-only git inspection is allowed (`git log`, `git blame`) when needed.
- Keep skill version checks low-noise:
  - Do not run `but skill check` as a routine preflight on every task.
  - Run `but skill check` when command behavior appears to diverge from this skill (for example: unexpected unknown-flag errors, missing subcommands, or output shape mismatches), or when the user asks.
  - If update is available, recommend `but skill check --update` (or run it if the user asked to update).
- For deeper command syntax and flags, use `references/reference.md`.
- For workspace model and dependency behavior, use `references/concepts.md`.
- For end-to-end workflow patterns, use `references/examples.md`.
