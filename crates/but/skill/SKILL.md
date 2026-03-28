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
2. Always add `--status-after` to mutation commands.
3. Use CLI IDs from `but status -fv` / `but diff` / `but show`; never hardcode IDs.
4. Start with `but status -fv` before mutations so IDs and stack state are current.
5. Create a branch for new work with `but branch new <name>` when needed.

## Core Flow

**Every write task** should follow this sequence.

```bash
# 1. Inspect state and gather IDs
but status -fv

# 2. If new branch needed:
but branch new <name>

# 3. Edit files (Edit/Write tools)

# 4. Refresh IDs if needed
but status -fv

# 5. Perform mutation with IDs from status/diff/show
but <mutation> ... --status-after
```

## Command Patterns

- Commit: `but commit <branch> -m "<msg>" --changes <id>,<id> --status-after`
- Commit + create branch: `but commit <branch> -c -m "<msg>" --changes <id> --status-after`
- Amend: `but amend <file-id> <commit-id> --status-after`
- Reorder commits: `but move <source-commit-id> <target-commit-id> --status-after` (**commit IDs**, not branch names)
- Stack branches: `but move <branch-name-or-id> <target-branch-name-or-id> --status-after` (**branch names or branch CLI IDs**)
- Tear off a branch: `but move <branch-name-or-id> zz --status-after` (`zz` = unassigned; branch name or branch CLI ID)
- Equivalent branch subcommand syntax remains available: `but branch move <branch-name> <target-branch-name>` and `but branch move --unstack <branch-name>`
- Push: `but push` or `but push <branch-id>`
- Pull: `but pull --check` then `but pull --status-after`

## Task Recipes

### Commit files

1. `but status -fv`
2. Find the CLI ID for each file you want to commit.
3. `but commit <branch> -m "<msg>" --changes <id1>,<id2> --status-after`
   Use `-c` to create the branch if it doesn't exist. Omit IDs you don't want committed.
4. **Check the `--status-after` output** for remaining uncommitted changes. If the file still appears as unassigned or assigned to another branch after commit, it may be dependency-locked. See "Stacked dependency / commit-lock recovery" below.

**If `but commit` fails with "invalid ID" or similar**: IDs can become stale when the workspace changes (e.g., after a branch move, or when multiple agents operate concurrently). Run `but status -fv` to get fresh IDs, then retry the commit.

### Amend into existing commit

1. `but status -fv` (or `but show <branch-id>`)
2. Locate file ID and target commit ID.
3. `but amend <file-id> <commit-id> --status-after`

### Reorder commits

`but move` supports both commit reordering and branch stack operations. Use commit IDs when reordering commits.

1. `but status -fv`
2. `but move <commit-a> <commit-b> --status-after` — uses commit IDs like `c3`, `c5`
3. Refresh IDs from the returned status, then run the inverse: `but move <commit-b> <commit-a> --status-after`

### Stack existing branches

To make one existing branch depend on (stack on top of) another, use top-level `move`:

```bash
but move feature/frontend feature/backend
```

This moves the frontend branch on top of the backend branch in one step.

Equivalent subcommand syntax:

```bash
but branch move feature/frontend feature/backend
```

**DO NOT** use `uncommit` + `branch delete` + `branch new -a` to stack existing branches. That approach fails because git branch names persist even after `but branch delete`. Always use `but move <branch> <target-branch>` (or the equivalent `but branch move ...`).

**To unstack** (make a stacked branch independent again):

```bash
but move feature/logging zz
```

Equivalent subcommand syntax:

```bash
but branch move --unstack feature/logging
```

**Note:** branch stack/tear-off operations use branch **names** (like `feature/frontend`) or branch CLI IDs, while commit reordering uses commit **IDs** (like `c3`). Do NOT use `but undo` to unstack — it may revert more than intended and lose commits.

### Stacked dependency / commit-lock recovery

A **dependency lock** occurs when a file was originally committed on branch A, but you're trying to commit changes to it on branch B. Symptoms:
- `but commit` succeeds but the file still appears in `unassignedChanges` in the `--status-after` output
- The file shows as "unassigned" instead of being staged to any branch

**Recovery:** Stack your branch on the dependency branch, then commit:

1. `but status -fv` — identify which branch originally owns the file (check commit history).
2. `but move <your-branch-name> <dependency-branch-name>` — stack your branch on the dependency. Uses full branch **names**, not CLI IDs.
3. `but status -fv` — the file should now be assignable. Commit it.
4. `but commit <branch> -m "<msg>" --changes <id> --status-after`

**If `but move <branch> <target-branch>` fails:** Do NOT try `uncommit`, `squash`, or `undo` to work around it — these will leave the workspace in a worse state. Instead, re-run `but status -fv` to confirm both branches still exist and are applied, then retry with exact branch names from the status output.

### Resolve conflicts after reorder/move

**NEVER use `git add`, `git commit`, `git checkout --theirs`, `git checkout --ours`, or any git write commands during resolution.** Only use `but resolve` commands and edit files directly with the Edit tool.

If `but move` causes conflicts (conflicted commits in status):

1. `but status -fv` — find commits marked as conflicted.
2. `but resolve <commit-id>` — enter resolution mode. This puts conflict markers in the files.
3. **Read the conflicted files** to see the `<<<<<<<` / `=======` / `>>>>>>>` markers.
4. **Edit the files** to resolve conflicts by choosing the correct content and removing markers.
5. `but resolve finish` — finalize. Do NOT run this without editing the files first.
6. Repeat for any remaining conflicted commits.

**Common mistakes:** Do NOT use `but amend` on conflicted commits (it won't work). Do NOT skip step 4 — you must actually edit the files to remove conflict markers before finishing.

## Git-to-But Map

| git | but |
|---|---|
| `git status` | `but status -fv` |
| `git add` + `git commit` | `but commit ... --changes ...` |
| `git checkout -b` | `but branch new <name>` |
| `git push` | `but push` |
| `git rebase -i` | `but move`, `but squash`, `but reword` |
| `git rebase --onto` | `but branch move <branch> <new-base>` |
| `git cherry-pick` | `but pick` |


## Cross-Branch Dependency Recovery

When `but commit` rejects some changes, the output lists which files failed and why:

```
Warning: Some selected changes could not be committed.
  ✗ src/shared/errors.ts (conflicts with changes on another branch)
Hint: run `but branch move auth shared` to stack auth on top of shared, then retry the commit
```

**Follow this workflow when you see rejected changes:**
1. Run the suggested `but branch move <your-branch> <dependency-branch>` command.
2. **Always** refresh IDs with `but status -fv` — branch move changes the workspace and invalidates old IDs.
3. Retry committing the previously rejected changes using the **new IDs** from step 2: `but commit <branch> -m "<msg>" --changes <new-id> --status-after`
4. **Verify success**: the output must include "✓ Created commit". If you only see warnings (no "✓ Created commit" line), zero changes were committed — do not assume success. Check `--status-after` for current state.

**When all selected changes are rejected**: `but commit` will only show warnings (no "✓ Created commit" line). This means zero changes were committed — do not assume success. Re-run `but status -fv` to see the current state and determine the correct next step.

**Critical**: After `but branch move`, old file IDs are invalid. Using old IDs causes the same rejection to repeat. Always refresh with `but status -fv` before retrying.

## Using `--status-after` Effectively

`--status-after` appends a **full workspace status** (including file IDs and branch topology) to the command output. This is equivalent to running `but status -fv` immediately after.

**Do NOT run `but status -fv` after a successful `--status-after` command.** The status is already in the output — read it directly.

**When you DO need a fresh status:**
- After a command that did NOT use `--status-after`
- After an error (the status may not have been appended)
- If significant time has passed or another session may have modified state

**Before/after example:**
```bash
# ❌ Wasteful — redundant status call
but commit auth -m "msg" --changes a1 --status-after
but status -fv   # ← unnecessary, data already in output above

# ✅ Efficient — use IDs from --status-after output directly
but commit auth -m "msg" --changes a1 --status-after
# Read file/branch IDs from the status output above
but commit auth -m "next change" --changes b2 --status-after
```

## Notes

- Prefer explicit IDs over file paths for mutations.
- `--changes` accepts comma-separated values (`--changes a1,b2`) or repeated flags (`--changes a1 --changes b2`), not space-separated.
- Read-only git inspection (`git log`, `git blame`, `git show --stat`) is allowed.
- After a successful `--status-after`, don't run a redundant `but status -fv` unless you need new IDs.
- Use `but show <branch-id>` to see commit details for a branch, including per-commit file changes and line counts.
- **Per-commit file counts**: `but status` does NOT include per-commit file counts. Use `but show <branch-id>` or `git show --stat <commit-hash>` to get them.
- Avoid `--help` probes; use this skill and `references/reference.md` first. Only use `--help` after a failed attempt.
- Run `but skill check` only when command behavior diverges from this skill, not as routine preflight.
- For command syntax and flags: `references/reference.md`
- For workspace model: `references/concepts.md`
- For workflow examples: `references/examples.md`
