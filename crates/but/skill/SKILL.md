---
name: but
version: 0.0.0
description: "Commit, push, branch, and manage version control with GitButler. Use for commits, selective dirty-file or hunk commits, branches, diffs, PRs, history edits, squashes, amends, undo, merge, apply, and unapply. For selected dirty files or hunks, inspect with `but diff`; do not run `but status` or `but status -fv` unless existing branch, stack, commit, conflict, or history context is needed. Replaces git write commands."
author: GitButler Team
---

# GitButler CLI Skill

Use GitButler CLI (`but`) as the default version-control interface.

## Non-Negotiable Rules

1. Use `but` for all write operations. Never run `git add`, `git commit`, `git push`, `git checkout`, `git merge`, `git rebase`, `git stash`, or `git cherry-pick`. If the user says a `git` write command, translate it to `but` and run that.
2. After mutations, read the returned output for the updated workspace state — it replaces a follow-up `but status -fv`.
3. Never chain `but` mutations with `&&` or `;`. Each mutation can reassign CLI IDs, so the second command may silently target the wrong file or commit. Run one mutation, read the returned workspace state, and take fresh IDs from it.
4. Use CLI IDs from `but diff` / `but status -fv` / `but show`; never hardcode IDs.
5. Do not run `but status` or `but status -fv` as routine preflight for selected dirty-file or hunk commits. Start with `but diff`; use `but status -fv` when existing branch, stack, commit, conflict, or history state matters.
6. For "commit these selected changes on a new branch", prefer one command: `but commit <branch> -c -m "<msg>" --changes <ids>`.

## Choose Inspection By Task

Start with the narrowest inspection that answers the task. Avoid ritual status checks.

```bash
# Selected dirty files/hunks:
but diff

# Branch/stack/commit/conflict/history state:
but status -fv

# Details for one known branch or commit:
but show <id>
```

Do not run plain `but status` and then `but status -fv`; that is usually a redundant round-trip.

Perform mutations with IDs from `diff`, `status -fv`, or `show`:

```bash
but <mutation> ...
```

## Command Patterns

- Commit: `but commit <branch> -m "<msg>" --changes <id>,<id>`
- `but commit -a` is accepted as a no-op compatibility flag; GitButler already includes uncommitted changes by default.
- Commit + create branch: `but commit <branch> -c -m "<msg>" --changes <id>`
- Amend: `but amend <commit-id> --changes <file-or-hunk-id>,<file-or-hunk-id>`
- Insert empty commit: `but commit empty [-m "<msg>"] [<target>]`
- Reorder commits: `but move <source-commit-id> <target-commit-id>` (**commit IDs**, not branch names)
- Stack branches: `but move <branch-name-or-id> <target-branch-name-or-id>` (**branch names or branch CLI IDs**)
- Tear off a branch: `but move <branch-name-or-id> zz` (`zz` = unassigned; branch name or branch CLI ID)
- Push: `but push <branch-name>` — always specify the branch; bare `but push` pushes ALL branches when run non-interactively
- Pull: `but pull --check` then `but pull`

## Task Recipes

### Update workspace from main

For "get latest from main", "update/sync this workspace", or "pull main":

1. `but status -fv`
2. `but pull --check`
3. If clean, `but pull`
4. `but status -fv`

`but pull` updates applied branches onto the latest target branch (usually
`main`). Do not use raw `git pull` or `git rebase`.

### Commit selected files or hunks

1. `but diff` — use this first for selective dirty commits. It shows file and hunk IDs for uncommitted changes.
2. Use file IDs when whole files belong in the commit. Use hunk IDs when only part of a file belongs. Do not run plain `but status` first.
3. For a new branch, use one command: `but commit <branch> -c -m "<msg>" --changes <id1>,<id2>`.
   For an existing branch, omit `-c`: `but commit <branch> -m "<msg>" --changes <id1>,<id2>`.
   Omit IDs you don't want committed.
   Creating a new branch with `-c` does not require a prior `but branch` or `but status -fv`.
4. **Check the returned status** for remaining uncommitted changes. If the file still appears as unassigned or assigned to another branch after commit, it may be dependency-locked. See "Stacked dependency / commit-lock recovery" below.

Edge case: if wanted and unwanted edits are in the same diff hunk, GitButler cannot split that hunk by ID. Only when the task requires keeping part of that hunk uncommitted, temporarily edit the working tree to isolate the wanted lines, commit with `--changes`, then restore the leftover lines so they remain uncommitted.

### Amend into existing commit

1. `but status -fv` (or `but show <branch-id>`)
2. Locate file/hunk IDs and target commit ID.
3. `but amend <commit-id> --changes <file-or-hunk-id>,<file-or-hunk-id>`; use one command for multiple files/hunks that belong in the same commit.

### Reorder commits

`but move` supports both commit reordering and branch stack operations. Use commit IDs when reordering commits.

1. `but status -fv`
2. `but move <commit-a> <commit-b>` — uses commit IDs like `c3`, `c5`
3. Refresh IDs from the returned status if you need to keep editing history.

### Stack existing branches

To make one existing branch depend on (stack on top of) another, use top-level `move`:

```bash
but move feature/frontend feature/backend
```

This moves the frontend branch on top of the backend branch in one step.

**DO NOT** use `uncommit` + `branch delete` + `branch new -a` to stack existing branches. That approach fails because git branch names persist even after `but branch delete`. Always use `but move <branch> <target-branch>`.

**To unstack** (make a stacked branch independent again):

```bash
but move feature/logging zz
```

**Note:** branch stack/tear-off operations use branch **names** (like `feature/frontend`) or branch CLI IDs, while commit reordering uses commit **IDs** (like `c3`). Do NOT use `but undo` to unstack — it may revert more than intended and lose commits.

### Stacked dependency / commit-lock recovery

A **dependency lock** occurs when a file was originally committed on branch A, but you're trying to commit changes to it on branch B. Symptoms:
- `but commit` succeeds but the file still appears in `unassignedChanges` in the returned status
- The file still shows as "unassigned" in the status output

**Recovery:** Stack your branch on the dependency branch, then commit:

1. `but status -fv` — identify which branch originally owns the file (check commit history).
2. `but move <your-branch-name> <dependency-branch-name>` — stack your branch on the dependency. Uses full branch **names**, not CLI IDs.
3. `but status -fv` — the file should now be committable. Commit it.
4. `but commit <branch> -m "<msg>" --changes <id>`

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
| `git status` | `but status -fv` for branch/stack state; `but diff` for selected dirty changes |
| `git add` + `git commit` | `but commit ... --changes ...` |
| `git checkout -b` + commit | `but commit <branch> -c -m ... --changes ...` |
| `git push` | `but push <branch-name>` |
| `git rebase -i` | `but move`, `but squash`, `but reword` |
| `git rebase --onto` | `but move <branch> <new-base>` |
| `git cherry-pick` | `but pick` |

## Notes

- Prefer explicit IDs over file paths for mutations.
- `--changes` accepts comma-separated values (`--changes a1,b2`) or repeated flags (`--changes a1 --changes b2`), not space-separated.
- Avoid plain `but status` in write flows. It is a compact human overview; agents usually need `but diff` or `but status -fv` next, so starting with plain status adds a redundant round-trip.
- Read-only git inspection (`git log`, `git blame`, `git show --stat`) is allowed.
- After a successful mutation, trust the workspace state it printed. Re-run `but status -fv` only if that output lacks the ID you need or files changed since.
- Use `but show <branch-id>` to see commit details for a branch, including per-commit file changes and line counts.
- **Per-commit file counts**: `but status` does NOT include per-commit file counts. Use `but show <branch-id>` or `git show --stat <commit-hash>` to get them.
- Avoid `--help` probes; use this skill and `references/reference.md` first. Only use `--help` after a command fails or required syntax is missing from the installed references.
- Run `but skill check` only when command behavior diverges from this skill, not as routine preflight.
- If `but` prints an `AGENT ACTION REQUIRED` skill warning, run the suggested command once, then reload/use the GitButler skill. If it repeats, report it instead of retrying.
- For command syntax and flags: `references/reference.md`
- For workspace model: `references/concepts.md`
- For workflow examples: `references/examples.md`
