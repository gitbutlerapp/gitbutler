---
name: but
version: 0.0.0
description: "Commit, push, branch, and manage version control with GitButler. Use for commits, selective dirty-file or hunk commits, branches, diffs, PRs, history edits, squashes, amends, undo, merge, apply, and unapply. For selected dirty files or hunks, inspect with `but diff`; use compact `but status` for commit order, branch/stack placement, or conflict overview; use `but status -fv` when file/hunk IDs or per-commit file details matter. Replaces git write commands."
author: GitButler Team
---

# GitButler CLI Skill

Use GitButler CLI (`but`) as the default version-control interface.

## Start Here

Choose the narrowest first command by task; avoid ritual status checks:

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

For "commit just/only/specific changes on a new branch", use the fast path:

```bash
but diff
but commit <branch> -c -m "<msg>" --changes <id>,<id>
```

`but commit <branch> -c ... --changes ...` creates the branch and prints the resulting workspace state. Do not run a separate `but branch new`, staging command, status command, or verification diff unless the returned output lacks information you need.

## IDs

The first token on each `but diff` / `but status` line is that line's ID — pass it to commands as-is; never hardcode or invent IDs. IDs may be a single character when unambiguous; copy them exactly from command output.

- `--changes` (or `-p`) takes comma-separated file or hunk IDs. A hunk ID is written `<file-id>:<hunk-id>` (e.g. `qs:5`, copied from `but diff`) — the part after the colon is the hunk's ID, **not** a line range (`qs:16-40` is invalid). Do not invent flags like `--hunk` / `--hunks` / `--ids`, pass a line range, or pass the IDs as positional arguments.
- Commit IDs are stable change IDs that survive history edits (`amend`, `squash`, `move`, `uncommit`, `reword`). Commits without a change ID (e.g. upstream-only) lead with a sha prefix instead, and `#N`-suffixed refs disambiguate duplicates — both go stale after history edits, and a stale sha can silently resolve to the wrong commit. The `(sha …)` on verbose commit lines is informational — do not pass it to commands.
- File/hunk IDs copied from one diff read generally remain usable across chained commits; branch IDs are stable. If an ID stops resolving, re-read `but status`/`but diff` and retry.

**Chaining:** you may chain mutations with `&&` off one inspection read. Chained `but commit` calls stack in the order written — the first is oldest, each later one goes on top. History edits may run in sequence when every commit ref involved is a change-ID ref; run them one at a time when a ref is sha-based or `#N`-suffixed, or when the next command needs IDs the previous one prints (e.g. recommitting after `but uncommit --diff`). Take follow-up refs from the returned workspace state.

## Non-Negotiable Rules

1. Use `but` for all write operations. Never run `git add`, `git commit`, `git push`, `git checkout`, `git merge`, `git rebase`, `git stash`, or `git cherry-pick`. If the user says a `git` write command, translate it to `but` and run that. Exceptions: `git add -- <path>` to mark a conflicted uncommitted file resolved (see "Conflicts in uncommitted files"), and a worktree-local Git commit when `but commit` reports that linked worktrees are unsupported. Never run `but setup` from a linked worktree.
2. After a mutation, read the workspace state it returned — it replaces a follow-up status command. Re-run `but status`/`but diff` only if that output lacks the ID you need or files changed since.
3. Never commit or push to a branch marked `(merged upstream)`; run `but pull` to remove it, or create/use another branch for new work.
4. In non-interactive CLI workflows, do not narrate progress between routine commands. Execute the needed `but` commands and give a concise final summary.
5. Avoid `--help` probes; use this skill and `references/reference.md` first. Only use `--help` after a command fails or required syntax is missing from the installed references.

## Command Patterns

- Commit: `but commit <branch> -m "<msg>" --changes <id>,<id>`
- Several commits from one diff: chain `but commit` calls with `&&` (commits stack oldest-first in the order written)
- Commit + create branch: `but commit <branch> -c -m "<msg>" --changes <id>`
- Commit at a specific history position: `... --before <commit-or-branch-id>` or `--after <commit-or-branch-id>`
- `but commit -a` is accepted as a no-op compatibility flag; GitButler already includes uncommitted changes by default.
- Amend: `but amend <commit-id> --changes <file-or-hunk-id>,<file-or-hunk-id>`
- Uncommit and show resulting dirty diff: `but uncommit <commit-id> --diff`
- Insert empty commit: `but commit empty [-m "<msg>"] [<target>]`
- Squash commits: `but squash <source-commit-id> [<source-commit-id>...] <target-commit-id> [-m "<msg>"]`
- Reorder commits: `but move <source-commit-id> <target-commit-id>` (**commit IDs**, not branch names)
- Reorder a block: `but move <source-commit-id>,<source-commit-id> <target-commit-id>` (comma-separated commit IDs)
- Move commit to branch top: `but move <commit-id> <branch-name-or-id>`
- Stack branches: `but move <branch-name-or-id> <target-branch-name-or-id>` (**branch names or branch CLI IDs**)
- Tear off a branch: `but move <branch-name-or-id> zz` (`zz` = uncommitted)
- Push: `but push <branch-name>` — always specify the branch; bare `but push` pushes ALL branches when run non-interactively
- Pull (update workspace from the target): `but pull` — the output reports the result; `--check` is only a dry-run preview
- Create PR: `but pr new <branch-id> [-m "Title..."] [-F pr_message.txt] [-t] [--draft]` — auto-pushes first; do not run `but push` before it

## Task Recipes

### Update workspace from main

For "get latest from main", "update/sync this workspace", "rebase onto main", or "pull main":

1. `but pull` — one command; no preflight needed. Its output reports the resulting state, it refuses safely when uncommitted changes conflict, and `but undo` reverts it.
2. If commits come back conflicted, resolve them oldest-first following the printed instructions: `but resolve <commit>`, edit the files, `but resolve finish`. Finishing a lower commit rebases the ones above it, so always work bottom-up.

Use `but pull --check` only to answer "would this conflict?" without updating (a dry-run preview), not as a step before every pull.

Rebasing applied branches onto the latest target IS `but pull` — never `move`, `config target`, `unapply`, or raw `git pull`/`git rebase`. The base shown in status is the last FETCHED state: when `git log` shows `main` (local or remote) ahead of it, that is exactly the update `but pull` fetches and applies — the target setting is not stale and repointing it is never the fix. Pull carries uncommitted changes along, and its output reports the resulting state. If it refuses because uncommitted changes conflict, park them as printed: `but commit <branch> --changes <ids> -m "wip"`, pull again, then `but uncommit` the parked commit (there is no stash; do not hand-revert files).

### Commit selected files or hunks

1. `but diff` — shows file and hunk IDs for uncommitted changes. Do not run plain `but status` first.
2. Use file IDs when whole files belong in the commit; use hunk IDs (`<file-id>:<hunk-id>`) when only part of a file belongs. Omit IDs you don't want committed.
3. New branch: `but commit <branch> -c -m "<msg>" --changes <id1>,<id2>` (no prior `but branch new` needed). Existing branch: omit `-c`.
4. **Check the returned status** for remaining uncommitted changes. If a file you committed still shows as uncommitted, it may be dependency-locked — see "Stacked dependency / commit-lock recovery".

Edge case: if wanted and unwanted edits are in the same diff hunk, GitButler cannot split that hunk by ID. Only when the task requires keeping part of that hunk uncommitted, temporarily edit the working tree to isolate the wanted lines, commit with `--changes`, then restore the leftover lines so they remain uncommitted.

### Amend into existing commit

1. `but status -fv` (or `but show <branch-id>`) — locate file/hunk IDs and target commit IDs.
2. `but amend <commit-id> --changes <id>,<id>` — one command per target commit. For several target commits, chain the amends with `&&` when every target is a change-ID ref; otherwise run them one at a time with fresh refs from each returned status.

### Split an existing commit

Use this when an existing commit should be replaced by selected smaller commits.

1. `but status -fv` when you need the source commit, branch name, or placement anchor.
2. `but uncommit <source-commit-id> --diff` exposes the commit's changes as uncommitted and prints committable file/hunk IDs.
3. Pick replacement contents from that dirty diff, not from the old committed diff.
4. Create the replacement commits oldest-first by chaining `but commit` calls. Each new commit goes to the TOP of the stack — if the split commit had commits above it, the replacements now sit above those preserved commits.
5. **If a commit must stay ABOVE the replacements, put it back on top instead of fighting anchors.** Do not anchor with `--before <top>`/`--after <top>` (sha/`#N` anchors go stale as each insert rewrites history; `--before <branch>` just puts commits on top of the branch). Take the preserved commit's ref from the last returned workspace state, then `but move <preserved-commit-id> <branch>` (several: `but move <id1>,<id2> <branch>`, oldest first).
6. Leave unwanted changes uncommitted. Remember the returned state lists commits newest first: replacements created oldest-first therefore appear in reverse request order under the preserved commits — that is correct; do not reorder them. Stop when the history matches.

### Reorder commits

`but status` displays commits newest/top first, while task specs often list history oldest to newest — translate before moving.

1. `but status` once to get commit IDs (use `-fv` only if you also need file details).
2. `but move <source> <target-commit>` places source immediately before target in oldest-to-newest history (directly below it in `but status`). `--after` places it immediately after (directly above). `but move <source> <branch>` moves it to branch top/newest.
3. For an adjacent block, keep the block's internal order and run ONE move: `but move <oldest-block-id>,<newest-block-id> <following-commit-id>` — do not sort the whole branch or move members one by one. After a successful block move, stop if the returned status shows the requested order; do not move the anchor or block members again.
4. For other reorders, make the smallest set of moves; prefer the default before/below form because it matches the status display.

### Squash commits

1. `but status` for commit IDs and order.
2. Put the result/target commit last: `but squash <source> [<source>...] <target> -m "<new message>"`.
3. Multiple independent groups may run in sequence off one status read (targets keep their change-ID refs); prefer newer/top groups first. Take fresh refs only when a ref is sha-based or `#N`-suffixed.
4. Stop when the returned status shows the requested history; do not re-verify with extra status calls.

### Stack existing branches

To make one existing branch depend on another: `but move <child-branch> <parent-branch>` (branch **names** or branch CLI IDs — commit reordering uses commit IDs). To unstack: `but move <branch> zz`.

**DO NOT** stack via `uncommit` + `branch delete` + `branch new -a` (git branch names persist after delete and it loses work), and do not use `but undo` to unstack.

### Create or manage pull requests

`but pr new <branch-id>` pushes the branch and creates the PR in one step — no prior `but push`. Provide `-F pr_message.txt`, `-t`, or `-m` with real newlines (zsh/bash: `-m $'Title\n\nBody'`) so no editor opens. If forge auth is missing, run `but config forge auth`.

For stacked branches `but pr` is mandatory (it sets PR bases and stack metadata; `gh pr create` breaks that). To publish a whole stack: `but pr new <top-branch-id> -t`. Manage with `but pr auto-merge|set-draft|set-ready <selector>`. See `references/reference.md` for details.

### Stacked dependency / commit-lock recovery

A **dependency lock**: a file was originally committed on branch A, and you're committing changes to it on branch B. Symptom: `but commit` succeeds but the file still shows as uncommitted in the returned status.

Recovery: stack your branch on the dependency branch, then commit:

1. `but status -fv` — identify which branch's commits own the file.
2. `but move <your-branch-name> <dependency-branch-name>` (full branch **names**).
3. Commit the file: `but commit <branch> -m "<msg>" --changes <id>`.

If that `but move` fails, do NOT try `uncommit`, `squash`, or `undo` as a workaround — re-run `but status -fv` to confirm both branches exist and are applied, then retry with exact branch names.

### Resolve conflicted commits (after pull, move, or reorder)

**NEVER use `git add`, `git commit`, `git checkout --theirs/--ours`, or any git write command during resolution.** Only `but resolve` commands plus direct file edits.

1. Find conflicted commits: the `but pull` summary lists them oldest-first; otherwise `but status` marks them.
2. `but resolve <commit-id>` — enters resolution mode and prints the conflict regions.
3. **Edit the files** to remove `<<<<<<<` / `=======` / `>>>>>>>` markers and keep the correct content. Do NOT skip this; do NOT use `but amend` on conflicted commits.
4. `but resolve finish` — reports leftover markers and surviving uncommitted changes; no follow-up status needed.
5. Repeat for remaining conflicted commits, oldest first — finishing a lower commit rebases the ones above it.

### Conflicts in uncommitted files

`but status` marks uncommitted files with unresolved merge conflicts `{conflicted}`; they are excluded from committable changes and outside `but resolve` mode. Choose the desired contents or delete the file, then `git add -- <path>` to mark it resolved (the one permitted `git add`).

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
| `gh pr create` | `but pr new <branch-id>` |

## Notes

- Read-only git inspection (`git log`, `git blame`, `git show --stat`) is allowed.
- If `but` prints an `AGENT ACTION REQUIRED` skill warning, run the suggested command once, then reload/use the GitButler skill. If it repeats, report it instead of retrying.
- For command syntax and flags: `references/reference.md`
- For workspace model: `references/concepts.md`
- For workflow examples: `references/examples.md`
