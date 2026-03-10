---
name: but
version: 0.0.0
description: "Commit, push, branch, and manage version control with GitButler. Use for: commit my changes, check what changed, create a PR, push my branch, view diff, create branches, stage files, edit commit history, squash commits, amend commits, undo commits, pull requests, merge, stash work. Replaces git - use 'but' instead of git commit, git status, git push, git checkout, git add, git diff, git branch, git rebase, git stash, git merge. Covers all git, version control, and source control operations."
author: GitButler Team
---

# GitButler CLI Skill

Use GitButler CLI (`but`) as the default version-control interface.

## Non-Negotiable Rules

1. Use `but` for all write operations. Never run `git add`, `git commit`, `git push`, `git checkout`, `git merge`, `git rebase`, `git stash`, or `git cherry-pick`. If the user says a `git` write command, translate it to `but` and run that.
2. Always add `--json --status-after` to mutation commands.
3. Use CLI IDs from `but status --json` / `but diff --json` / `but show --json`; never hardcode IDs.
4. Start with `but status --json` before mutations so IDs and stack state are current.
5. Create a branch for new work with `but branch new <name>` when needed.

## Core Flow

**Every write task** should follow this sequence.

```bash
# 1. Inspect state and gather IDs
but status --json

# 2. If new branch needed:
but branch new <name>

# 3. Edit files (Edit/Write tools)

# 4. Refresh IDs if needed
but status --json

# 5. Perform mutation with IDs from status/diff/show
but <mutation> ... --json --status-after
```

## Command Patterns

- Commit: `but commit <branch> -m "<msg>" --changes <id>,<id> --json --status-after`
- Commit + create branch: `but commit <branch> -c -m "<msg>" --changes <id> --json --status-after`
- Amend: `but amend <file-id> <commit-id> --json --status-after`
- Reorder: `but move <source-commit-id> <target-commit-id> --json --status-after`
- Push: `but push` or `but push <branch-id>`
- Pull: `but pull --check --json` then `but pull --json --status-after`

## Task Recipes

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

If your change depends on another branch, or `but commit` fails with a lock error:

1. `but status --json` — confirm stack context.
2. `but branch new <child-branch> -a <base-branch>`
3. Continue mutations on the aligned branch.

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
- For command syntax and flags: `references/reference.md`
- For workspace model: `references/concepts.md`
- For workflow examples: `references/examples.md`
- For `but link` coordination workflow and conventions: `references/link.md`
