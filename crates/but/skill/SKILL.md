---
name: but
version: 0.0.0
description: "Commit, push, branch, and manage version control with GitButler. Use for commits, selective dirty-file or hunk commits, branches, diffs, PRs, history edits, squashes, amends, undo, merge, apply, and unapply. For selected dirty files or hunks, inspect with `but diff`; use compact `but status` for commit order, branch/stack placement, or conflict overview; use `but status -fv` when file/hunk IDs or per-commit file details matter. Replaces git write commands."
author: GitButler Team
---

# GitButler CLI Skill

Use GitButler CLI (`but`) as the default version-control interface.

## Non-Negotiable Rules

1. Use `but` for all write operations. Never run `git add`, `git commit`, `git push`, `git checkout`, `git merge`, `git rebase`, `git stash`, or `git cherry-pick`. If the user says a `git` write command, translate it to `but` and run that.
2. After mutations, read the returned output for the updated workspace state — it replaces a follow-up status command.
3. Never chain `but` mutations with `&&` or `;`. Each mutation can reassign CLI IDs, so the second command may silently target the wrong file or commit. Run one mutation, read the returned workspace state, and take fresh IDs from it.
4. Use CLI IDs from `but diff` / `but status` / `but status -fv` / `but show`; never hardcode IDs.
5. Do not run `but status` or `but status -fv` as routine preflight for selected dirty-file or hunk commits. Start with `but diff`; use compact `but status` for commit order, branch/stack placement, or conflict overview. Use `but status -fv` when file/hunk IDs or per-commit file details matter.
6. For "commit these selected changes on a new branch", prefer one command: `but commit <branch> -c -m "<msg>" --changes <ids>`.
7. In non-interactive CLI workflows, do not narrate progress between routine commands. Execute the needed `but` commands and give a concise final summary.

## Choose Inspection By Task

Start with the narrowest inspection that answers the task. Avoid ritual status checks.

```bash
# Selected dirty files/hunks:
but diff

# Commit order, branch/stack placement, conflict overview:
but status

# File/hunk IDs, per-commit files, amend/split details:
but status -fv

# Details for one known branch or commit:
but show <id>
```

Do not run plain `but status` and then `but status -fv` unless the compact output lacks file/hunk details needed for the task.

Perform mutations with IDs from `diff`, `status`, `status -fv`, or `show`:

```bash
but <mutation> ...
```

## Command Patterns

- Commit: `but commit <branch> -m "<msg>" --changes <id>,<id>`
- Batch selected commits: `but commit batch <branch> [--before <target>|--after <target>] -m "<msg>" --changes <id>,<id> -m "<msg>" --changes <id>,<id>`
- `but commit -a` is accepted as a no-op compatibility flag; GitButler already includes uncommitted changes by default.
- Commit + create branch: `but commit <branch> -c -m "<msg>" --changes <id>`
- Commit at a specific history position: `but commit <branch> -m "<msg>" --changes <id>,<id> --before <commit-or-branch-id>` or `--after <commit-or-branch-id>`
- Amend: `but amend <commit-id> --changes <file-or-hunk-id>,<file-or-hunk-id>`
- Uncommit and show resulting dirty diff: `but uncommit <commit-id> --diff`
- Insert empty commit: `but commit empty [-m "<msg>"] [<target>]`
- Squash commits: `but squash <source-commit-id> [<source-commit-id>...] <target-commit-id> [-m "<msg>"]`
- Reorder commits: `but move <source-commit-id> <target-commit-id>` (**commit IDs**, not branch names)
- Reorder multiple commits as a group: `but move <source-commit-id>,<source-commit-id> <target-commit-id>` (comma-separated commit IDs)
- Move commit to branch top: `but move <commit-id> <branch-name-or-id>` (puts that commit at the top/newest position of the branch)
- Move multiple commits to branch top: `but move <commit-id>,<commit-id> <branch-name-or-id>` (commit IDs only; not branches)
- Stack branches: `but move <branch-name-or-id> <target-branch-name-or-id>` (**branch names or branch CLI IDs**)
- Tear off a branch: `but move <branch-name-or-id> zz` (`zz` = uncommitted; branch name or branch CLI ID)
- Push: `but push <branch-name>` — always specify the branch; bare `but push` pushes ALL branches when run non-interactively
- Pull: `but pull --check` then `but pull`
- Create PR: `but pr new <branch-id> [-m "Title..."] [-F pr_message.txt] [-t] [--draft]` — auto-pushes before creating the PR; do not run `but push` first
- Manage PRs: `but pr auto-merge <selector>`, `but pr set-draft <selector>`, `but pr set-ready <selector>`

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
4. **Check the returned status** for remaining uncommitted changes. If the file still appears as uncommitted or assigned to another branch after commit, it may be dependency-locked. See "Stacked dependency / commit-lock recovery" below.

Edge case: if wanted and unwanted edits are in the same diff hunk, GitButler cannot split that hunk by ID. Only when the task requires keeping part of that hunk uncommitted, temporarily edit the working tree to isolate the wanted lines, commit with `--changes`, then restore the leftover lines so they remain uncommitted.

### Amend into existing commit

1. `but status -fv` (or `but show <branch-id>`)
2. Locate file/hunk IDs and target commit ID.
3. `but amend <commit-id> --changes <file-or-hunk-id>,<file-or-hunk-id>`; use one command for multiple files/hunks that belong in the same commit.

### Split an existing commit

Use this when an existing commit should be replaced by selected smaller commits.

1. `but status -fv` when you need the source commit, branch name, or placement anchor.
2. `but uncommit <source-commit-id> --diff` to expose that commit's changes as uncommitted changes and print committable file/hunk IDs.
3. Pick replacement commit contents from the dirty diff printed by `but uncommit --diff`, not from the old committed diff.
4. For multiple replacements from the same diff, prefer one batch command:
   `but commit batch <branch> [--before <target>|--after <target>] -m "<message 1>" --changes <id>,<id> -m "<message 2>" --changes <id>,<id>`
   Each message pairs with the changes group at the same occurrence index. Use comma-separated IDs inside each `--changes` group. Order batch entries in history order, oldest to newest; when inserting before a newer anchor, the last batch entry lands nearest that anchor.
5. Leave unwanted changes uncommitted. If the returned workspace state shows the requested commits and leftovers, stop; do not run `status`, `diff`, `show`, or `--help` only to reconfirm.

### Reorder commits

`but move` supports both commit reordering and branch stack operations. Use commit IDs when reordering commits.
`but status` displays commits newest/top first, while task specs often list history oldest to newest. Use `but status -fv` only if you also need file-level details.

1. `but status`
2. `but move <source-commit-id> <target-commit-id>` places source immediately before target in oldest-to-newest history. In `but status`, source appears directly below target.
3. `but move <source-commit-id> <target-commit-id> --after` places source immediately after target in oldest-to-newest history. In `but status`, source appears directly above target.
4. `but move <source-commit-id> <branch-name-or-id>` moves source to branch top/newest.
5. `but move <source-commit-id>,<source-commit-id> <target-commit-id>` moves multiple commit sources together. Multi-source moves accept comma-separated commit IDs only, not branch names.

For explicit final-order tasks, translate the requested order into the newest-to-oldest order shown by `but status`, then make the smallest set of moves. Prefer the default before/below form because it matches the status display: `but move <source> <newer-neighbor>` places source directly below its newer neighbor. Use a branch target only when a commit must become top/newest, and use `--after` only when it clearly avoids extra moves.

### Squash commits

Use this when multiple existing commits should become one commit.

1. `but status` to get commit IDs and current order. Use `but status -fv` only if you also need per-commit file details.
2. Put the result/target commit last: `but squash <source-commit-id> [<source-commit-id>...] <target-commit-id> -m "<new message>"`.
3. For multiple independent squash groups, prefer newer/top groups first; history edits can rewrite IDs above the edited commit, so use returned status before the next squash.
4. After the final squash, stop if returned status shows the requested history; do not run `--help`, `status`, or `status -fv` only to reconfirm.

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

### Create or manage pull requests

`but pr new <branch-id>` pushes the branch and creates a pull request in one step. Do not run `but push` first. Agents must provide `-F pr_message.txt`, `-t`, or `-m` with real newline characters (zsh/bash: `-m $'Title\n\nBody'`) so the command does not open an editor. If forge auth is missing, run `but config forge auth`.

For independent, unstacked branches, another forge tool may be okay if the user asked for it or `but pr` is unavailable. Still prefer `but pr new` when you are already working from GitButler branch IDs.

For stacked branches, `but pr` is mandatory. Create and manage stacked PRs with GitButler so PR bases match the stack and GitButler can maintain the stack metadata in PR description footers. Do not create stacked PRs with `gh pr create` or other forge tools.

To publish a whole stack, create the PR from the top branch you want published:

```bash
but pr new <top-branch-id> -t
```

That creates or updates reviews for that branch and its dependencies in stack order. Use `but pr auto-merge <selector>`, `but pr set-draft <selector>`, and `but pr set-ready <selector>` for follow-up PR management; selectors can be branch names, branch IDs, stack IDs, or numeric review IDs.

### Stacked dependency / commit-lock recovery

A **dependency lock** occurs when a file was originally committed on branch A, but you're trying to commit changes to it on branch B. Symptoms:
- `but commit` succeeds but the file still appears in `uncommittedChanges` in the returned status
- The file still shows as "uncommitted" in the status output

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
| `git status` | `but status` for branch/stack/commit overview; `but status -fv` for file/hunk details; `but diff` for selected dirty changes |
| `git add` + `git commit` | `but commit ... --changes ...` |
| `git checkout -b` + commit | `but commit <branch> -c -m ... --changes ...` |
| `git push` | `but push <branch-name>` |
| `git rebase -i` | `but move`, `but squash`, `but reword` |
| `git rebase --onto` | `but move <branch> <new-base>` |
| `git cherry-pick` | `but pick` |
| `gh pr create` | `but pr new <branch-id>` (pushes the branch, then creates the PR; no separate `but push`) |

## Notes

- Use plain `but status` for commit order, branch/stack placement, and conflict overview. Escalate to `but status -fv` only when file/hunk IDs or per-commit file details are needed.
- Read-only git inspection (`git log`, `git blame`, `git show --stat`) is allowed.
- After a successful mutation, trust the workspace state it printed. Re-run `but status` or `but status -fv` only if that output lacks the ID you need or files changed since.
- Avoid `--help` probes; use this skill and `references/reference.md` first. Only use `--help` after a command fails or required syntax is missing from the installed references.
- If `but` prints an `AGENT ACTION REQUIRED` skill warning, run the suggested command once, then reload/use the GitButler skill. If it repeats, report it instead of retrying.
- For command syntax and flags: `references/reference.md`
- For workspace model: `references/concepts.md`
- For workflow examples: `references/examples.md`
