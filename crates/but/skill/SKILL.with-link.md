---
name: but
version: 0.0.0
description: "Commit, push, branch, and manage version control with GitButler. Use for: commit my changes, check what changed, create a PR, push my branch, view diff, create branches, stage files, edit commit history, squash commits, amend commits, undo commits, pull requests, merge, stash work. Replaces git - use 'but' instead of git commit, git status, git push, git checkout, git add, git diff, git branch, git rebase, git stash, git merge. Covers all git, version control, and source control operations. Also coordinates file edits between agents - load this skill before ANY file edit, not just version control tasks."
author: GitButler Team
---

# GitButler CLI Skill

Use GitButler CLI (`but`) as the default version-control interface.

## Non-Negotiable Rules

1. Use `but` for all write operations. Never run `git add`, `git commit`, `git push`, `git checkout`, `git merge`, `git rebase`, `git stash`, or `git cherry-pick`. If the user says a `git` write command, translate it to `but` and run that.
2. Always add `--json --status-after` to mutation commands.
3. Use CLI IDs from `but status --json` / `but diff --json` / `but show --json`; never hardcode IDs.
4. Before any file edit, coordinate via `but link` (read → check → claim → edit → done). No exceptions for "small" changes. See Core Flow.
5. Use one stable `agent-id` per task on every `but link` command. Batch file paths when possible.
6. For review/analysis tasks with no file edits, still coordinate: post once at start and post after each meaningful review pass.

## Core Flow

**Every task that edits files** must follow this sequence.

```bash
# 1. Read channel state — this is NOT optional
but link read --agent-id <id>
# You MUST read recent messages, adapt your plan, AND reply:
#   - If another agent posted "done: <X>", do NOT redo that work.
#   - If a discovery warns about a file, factor that into your approach.
#   - If an intent overlaps with your planned changes, coordinate first.
#   - If another agent shared analysis, a triage, or findings relevant
#     to your work, reply with your perspective or an ack.
# The channel is a team conversation, not just a lock protocol.
# Ignoring what others have posted is a coordination failure.

# 2. Coordinate BEFORE touching any file
but link check <file1>,<file2> --agent-id <id>
but link claim <file1>,<file2> --agent-id <id>

# 3. Status
but status --json

# 4. If new branch needed:
but branch new <name>

# 5. Edit files (Edit/Write tools)

# 6. Perform mutation with IDs from status/diff/show
but <mutation> ... --json --status-after

# 7. Signal completion (auto-releases all claims)
but link done "<summary of what changed>" --agent-id <id>
```

If you are about to use the Edit or Write tool and have NOT run `but link check` + `but link claim` for that file yet, STOP and run them first.

If a file is blocked by another agent, skip it and post one coordination update; do not retry in a loop.

## Typed Messages

Use typed messages when your work affects other agents or when you discover important context.

### Discovery: "I found something important"

Post when you discover a risk, breaking change, or important context that other agents need.

```bash
but link discovery "breaking rename in types.rs" \
  --evidence "AuthToken renamed to SessionToken" \
  --evidence "3 callers in src/api.rs still use old name" \
  --action "but link check src/types.rs,src/api.rs --agent-id <id>" \
  --agent-id <id>
```

When to use:
- You find a file mid-refactor that other agents might touch
- A breaking API change is in progress
- A test suite is failing due to a known issue

### Intent: "I'm about to work on this API"

Post before making changes that affect shared API surfaces. Enables dependency-hint detection in `but link check`.

```bash
but link intent "crate::auth" \
  --tag api --surface AuthToken --surface verify_token \
  --agent-id <id>
```

When to use:
- Before changing function signatures that other code calls
- Before modifying shared types or traits

### Declare: "I own this API surface"

Post to declare ownership of an API contract so other agents get dependency hints.

```bash
but link declare "crate::auth" \
  --tag api --surface AuthToken --surface verify_token \
  --agent-id <id>
```

When to use:
- You are the author of a module's public API
- You want agents consuming your API to coordinate before changing it

## Command Patterns

- Commit: `but commit <branch> -m "<msg>" --changes <id>,<id> --json --status-after`
- Commit + create branch: `but commit <branch> -c -m "<msg>" --changes <id> --json --status-after`
- Amend: `but amend <file-id> <commit-id> --json --status-after`
- Reorder: `but move <source-commit-id> <target-commit-id> --json --status-after`
- Push: `but push` or `but push <branch-id>`
- Pull: `but pull --check --json` then `but pull --json --status-after`

## Task Recipes

### Edit a file (any file, any size change)

This is the most common recipe. Use it for ALL file edits — even single-line changes.

1. `but link read --agent-id <id>` — read recent messages. If another agent already completed work that overlaps with yours, adjust your plan. Do not repeat or overwrite their work.
2. `but link check <file> --agent-id <id>`
3. `but link claim <file> --agent-id <id> --ttl 15m`
4. Edit the file (Edit/Write tool)
5. `but link done "<what changed>" --agent-id <id>` — auto-releases all claims

Skipping this because the edit is "trivial" is the #1 coordination failure mode.

### Review / triage task (no file edits)

Use this for PR review, comment triage, investigation, or validation-only work.

1. `but link read --agent-id <id>` — sync channel state first.
2. `but link post "<what you are reviewing>" --agent-id <id>` — announce start.
3. Perform one review pass (for example: fetch comments, inspect code, validate behavior).
4. `but link post "<pass summary: agree/disagree + key findings>" --agent-id <id>` — required after each meaningful pass.
5. If the user asks for another pass, repeat steps 3-4.

### Commit files

1. `but status --json`
2. Find the `cliId` for each file you want to commit.
3. `but commit <branch> -m "<msg>" --changes <id1>,<id2> --json --status-after`
   Use `-c` to create the branch if it doesn't exist. Omit IDs you don't want committed.

### Amend into existing commit

1. `but status --json` (or `but show <branch-id> --json`)
2. Locate file ID and target commit ID.
3. `but amend <file-id> <commit-id> --json --status-after`

### Reorder commits

1. `but status --json`
2. `but move <commit-a> <commit-b> --json --status-after`
3. Refresh IDs from the returned status, then run the inverse: `but move <commit-b> <commit-a> --json --status-after`

### Stacked dependency / commit-lock recovery

If your change depends on another agent's branch, or `but commit` fails with a lock error:

1. `but status --json` — confirm stack context.
2. `but link post "Blocked on <base-branch>: <reason>" --agent-id <id>`
3. `but branch new <child-branch> -a <base-branch>`
4. Continue mutations on the aligned branch.

## Git-to-But Map

| git | but |
|---|---|
| `git status` | `but status --json` |
| `git add` + `git commit` | `but commit ... --changes ...` |
| `git checkout -b` | `but branch new <name>` |
| `git push` | `but push` |
| `git rebase -i` | `but move`, `but squash`, `but reword` |
| `git cherry-pick` | `but pick` |

## Notes

- Prefer explicit IDs over file paths for mutations.
- `--changes` accepts comma-separated values (`--changes a1,b2`) or repeated flags (`--changes a1 --changes b2`), not space-separated.
- Read-only git inspection (`git log`, `git blame`) is allowed.
- After a successful `--status-after`, don't run a redundant `but status` unless you need new IDs.
- Avoid `--help` probes; use this skill and `references/reference.md` first. Only use `--help` after a failed attempt.
- Run `but skill check` only when command behavior diverges from this skill, not as routine preflight.
- For coordination protocol details: `references/link.md`
- For command syntax and flags: `references/reference.md`
- For workspace model: `references/concepts.md`
- For workflow examples: `references/examples.md`
