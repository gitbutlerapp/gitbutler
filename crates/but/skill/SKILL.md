---
name: but
version: 0.0.0
description: "Commit, push, branch, and manage version control with GitButler. Use for commits, selective dirty-file or hunk commits, branches, diffs, PRs, history edits, squashes, amends, undo, merge, fetching, pulling and syncing a branch with its remote, apply, unapply, and linked worktrees. For selected dirty files or hunks, inspect with `but diff`; use compact `but status` for commit order, branch/stack placement, or conflict overview; use `but status -fv` when file IDs or per-commit file details matter. Replaces git write commands, including fetch and pull. Applies in every checkout of the repository, even when a plain branch is checked out, including linked worktrees such as `.claude/worktrees/*`, and to git writes made while testing or cleaning up (stash, checkout -- <file>, restore, reset)."
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

# File IDs, per-commit files, amend details:
but status -fv

# What one applied branch or commit changed, for review:
but diff <branch-or-commit>

# Details for one known branch or commit:
but show <id>
```

Do not run plain `but status` and then `but status -fv` unless the compact output lacks file details needed for the task.

For "commit just/only/specific changes on a new branch", use the fast path:

```bash
but diff
but commit -b <branch> -m "<msg>" <id> <id>
```

`but commit -b <branch> ...` creates the branch when it does not exist and prints the created commit. Do not run a separate `but branch new`, status command, or verification diff unless the task needs workspace information that the result does not provide.

In a linked worktree, such as `.claude/worktrees/*`, omit `-b` so the commit goes on the branch the worktree has checked out. Use that branch as the new branch; do not stack another one on it.

## IDs

The first token on each `but diff` / `but status` line is that line's ID. When a command needs an ID, copy it exactly from the current output; never hardcode or invent one. ID lifetimes differ by entity:

- Changes and sources are **positional, space-separated** IDs (`but commit -b feat -m "msg" uvw:2e4 uvw:e2c`). An uncommitted hunk ID is written `<file-id>:<hunk-id>` (e.g. `uvw:2e4`, copied from bare `but diff`) — the part after the colon is the hunk's ID, **not** a line range (`uvw:16-40` is invalid). Do not invent flags like `--changes` / `--hunk` / `--ids`, pass a line range, or comma-separate IDs — `uvw,qyo` is parsed as one ID and fails.
- `but diff` is the exception: it accepts at most **one** target. Bare `but diff` shows all uncommitted files; inspect committed files or other entities one target at a time — never `but diff <id> <id>`.
- A committed file is `<commit-id>:<file-id>` (e.g. `mzm:uvw`, shown under each commit). A committed hunk is `<commit-id>:<file-id>:<hunk-id>` and is shown by `but diff <commit-id>`. `@` means the uncommitted area.
- Commit IDs are stable change IDs that survive history edits (`amend`, `squash`, `move`, `uncommit`, `reword`). Commits without a change ID (e.g. upstream-only) lead with a sha prefix instead, and `#N`-suffixed refs disambiguate duplicates — both go stale after history edits, and a stale sha can silently resolve to the wrong commit. The `(sha …)` on verbose commit lines is informational — do not pass it to commands.
- Branch short IDs are snapshot-local selectors. Use full branch names for branch-targeting mutations; short IDs are safe only for immediate read-only inspection.
- File/hunk IDs copied from one diff read generally remain usable across chained commits. If one stops resolving, re-read `but diff` and retry.

**Chaining:** mutation output is concise by default, so you may chain mutations with `&&` off one inspection read. Do not chain branch short IDs through mutations; use full branch names. Add `--status-after` only when the next step needs workspace IDs or details that the mutation result does not provide. Chained `but commit` calls stack in the order written — the first is oldest, each later one goes on top. History edits may run in sequence when every commit ref involved is a change-ID ref; run them one at a time with `--status-after` when a ref is sha-based or `#N`-suffixed, or when the next command needs freshly issued IDs. Chaining `but uncommit <id> && but diff` is safe because bare `diff` needs no ID from the uncommit output. Never pipe a mutation through `tail` or `head`: the pipeline then exits with their status, so the next `&&` runs even when the mutation failed.

## Non-Negotiable Rules

1. Use `but` for all write operations. Never run `git add`, `git commit`, `git push`, `git fetch`, `git pull`, `git checkout`, `git merge`, `git rebase`, `git stash`, or `git cherry-pick`; `but pull --check` fetches. The only exceptions are selecting a side of a conflicted submodule in resolution mode (see Resolve conflicted commits) and creating a missing target branch (see Notes). If the user says a `git` write command, translate it to `but` and run that.
2. Mutation commands print their result without appending workspace status. Add `--status-after` only when the next step needs resulting workspace IDs or details; otherwise trust the mutation result and do not run a verification status/diff.
3. Branches marked `(merged upstream)` have landed; start new work on another branch. That marker alone does not mean `but pull` removes them: run `but pull --check` first. `but pull` removes only branches listed `[integrated]` ("has been integrated upstream and removed locally", or "has been merged into <target> and removed" when the target has no upstream) and rebases the rest, which stay applied. To clear a landed branch that stays applied, use `but unapply <branch>` (acts on its whole stack) rather than `but discard`. `push` and mutations (`commit`, `amend`, `squash`, `uncommit`, `reword`, `move`) refuse landed branches and commits, `absorb` skips them with a notice, and `commit` skips them when picking a default target.
4. Keep going between routine `but` commands. Put any status note in the same message as the next command, and don't end a turn on a summary that announces the next `but` step without running it. Finish with a concise summary.
5. Prefer this skill and `but skill reference` over exploratory help calls. Use `<command> --help` when required syntax is missing or a command fails; use top-level help only when you genuinely need to discover an undocumented command.
6. Other agents may share this checkout, so `@` can hold their edits. Commit, amend, and discard only the IDs or paths you changed. Bare `but commit`, `but absorb`, `but discard @`, and `git stash` take their work too, and a refusal naming files you didn't edit means you selected theirs.

## Command Patterns

- Initialize a Git repository: `but init` (current directory) or `but -C <path> init`. Non-interactive; creates no commits, remotes, or GitButler workspace. Existing repositories are left unchanged. In a repository with no commits, make the first commit with a bare `but commit -m "<msg>"`; `-b` is refused there.

- Commit: `but commit -b <branch> -m "<msg>" <id> <id>` — `-b <branch>` creates the branch if it does not exist
- Commit everything uncommitted: `but commit -b <branch> -m "<msg>"` (omit the IDs)
- Several commits from one diff: chain `but commit` calls with `&&` (commits stack oldest-first)
- Never pass `-s` to `commit`, `move`, or `pick`: it is `--switch`, not git's `--signoff`, and it unapplies every other stack.
- The branch is never positional (`but commit <branch>` fails; use `-b`), `but amend` always needs `-t`, and `but diff` takes one target (use a shared `dir/` or one call each).
- Commit at a specific history position: `--above <commit-or-branch>` or `--below <commit-or-branch>` (mutually exclusive). With a branch target, add `-b <new-name>` to name the new branch; otherwise its name is generated. The name must not already exist. Do not combine `-b` (even without a name) with commit/worktree targets.
- Targeting is **required** when more than one **stack** is applied; without it `but commit` fails with "Unclear where to commit. Found more than one stack". Several branches stacked together count as one stack — an untargeted commit then silently lands on the stack's top branch, so pass `-b` whenever the branch matters.
- Always pass `-m "<msg>"` (or `-F -` with a heredoc, `-F <file>` with a file outside the repository, or `--no-message`) to `but commit`, and to `but squash` whenever its sources are commits or branches unless the target is `@` (squash also takes `-u` to keep the target's message) — those compose a new message; without a flag an agent run keeps whatever the command composed (empty for `but commit`, the joined source messages for `but squash`) and a terminal opens an editor. Squash sources that are uncommitted or committed changes reuse the target's message and need no flag; squashing into `@` rejects message flags outright.
- Amend: `but amend -t <commit-or-branch> <file-or-hunk-id> <file-or-hunk-id>` — a branch target resolves to its newest commit
- Uncommit: `but uncommit <commit-id>` (whole commit), `but uncommit <branch>` (all commits and remove branch), `but uncommit <commit-id>:<file-id>` (committed file), or `but uncommit <commit-id>:<file-id>:<hunk-id>` (committed hunk); committed files and hunks may be mixed, but all must come from one commit
- Copy commits (sources stay unchanged): `but pick <sha> [<sha>...] -b <new-branch>` puts the copies on a new independent branch from the target, `-b <existing-branch>` on that applied branch's tip, and `--above <branch> -b <new-name>` (or `--below`) on a new branch stacked there. Pick applies sources in the order given, so pass them oldest first. For a branch that is not applied, list SHAs with `git log --reverse <target>..<branch>` and inspect them with `git show` or `but show <sha>`; `but diff` and the IDs that `but branch show` prints for it do not work.
- Move committed changes into an existing commit or `@`: `but squash <commit-id>:<file-id> -t <commit-id-or-@>` for a committed file; `but squash <commit-id>:<file-id>:<hunk-id> -t <commit-id-or-@>` for a committed hunk; all sources must come from one commit
- Move committed changes into a new commit at a chosen position: `but move <commit-id>:<file-id> --above <commit-or-branch> -m "<msg>"` for a committed file; `but move <commit-id>:<file-id>:<hunk-id> --above <commit-or-branch> -m "<msg>"` for a committed hunk (`--below`, `--branch`, and `--unstack` also work)
- `but split` and a `but move` of committed changes name their new commit with `-m` or `-F` too; without either the commit gets an empty message. `but move` rejects `-m`/`-F` for whole-commit and branch sources.
- Move commit to branch top: `but move <commit-id> -b <branch>`
- Move commits or committed changes onto a new named branch: `but move <source> --above <branch> -b <new-name>` (`--below` also works); use `but move <source> --unstack -b <new-name>` for an independent branch. Omit `-b` for a generated name. Naming is not supported when stacking or unstacking a branch source.
- Discard: `but discard <id> [<id>...]` — accepts branches, commits, committed changes, uncommitted changes, or `@` for all uncommitted changes
- Push: `but push <top-branch>` — pushes the selected branch and its ancestors; to update a stack, select its top branch once and never loop. Bare `but push` pushes all unpushed work when run non-interactively — one push per stack (its topmost unpushed branch, ancestors included), and the output lists every branch it pushed. It exits non-zero if any stack failed; stacks that already pushed stay pushed, and rerunning after fixing the failure is safe (up-to-date stacks are skipped). `but push` and `but pr new` replace the remote branch: if others may have pushed to it, bring their commits in first (see Bring in commits pushed to a branch's own remote).
- Create PR: `but pr new <branch-name> [-m "Title..."] [-F - | -F <file>] [-t] [--draft]` — auto-pushes first; do not run `but push` before it

## Task Recipes

### Update workspace from main

For "get latest from main", "update/sync this workspace", "rebase onto main", or "pull main":

1. `but pull` — one command; no preflight needed. Its output reports the resulting state, and `but undo` reverts it. Uncommitted changes that conflict with the update get conflict markers and show as `{conflicted}`; resolve them as in Conflicts in uncommitted files.
2. If commits come back conflicted, resolve them with the `but resolve conflicts <branch>` and `but resolve apply` loop in Resolve conflicted commits, one branch at a time. When `HEAD` is `gitbutler/workspace`, `but pull` prints `but resolve <commit>` as the next step; that is edit mode, which is only for running code against a resolution.

`but pull --check` answers "would this conflict?" without updating. Do not use it as a routine
preflight; use it when the user asks for a preview, you need a fetch, repository policy requires one, other agents'
branches may move, or an applied branch was just merged upstream (see rule 3).

Rebasing applied branches onto the latest target IS `but pull` — never `move`, `config target`, `unapply`, or raw `git pull`/`git rebase`. The base shown in status is the last FETCHED state: when `git log` shows the remote target (for example `origin/main`) ahead of it, that is exactly the update `but pull` fetches and applies — the target setting is not stale and repointing it is never the fix. Pull carries uncommitted changes along; there is no stash, so do not hand-revert files.

### Bring in commits pushed to a branch's own remote

`but pull` only rebases onto the target. Commits someone pushed to your branch's remote show in `but status` under `(upstream: on <remote>/<branch>)`, above the branch's own commits; if status may be stale, run `but pull --check` to fetch.

1. `but branch update <branch>`, once, on the branch they pushed to.
2. Then `but pull` if the target moved too. Update before pulling. If the branch was rebased since its last push (for example by an earlier `but pull`), the update leaves an empty `(no changes)` copy of a shared commit. On a single branch, `but discard` that copy by its `#N` ref. In a stack, `but undo` the update and tell the user, because the update can also move commits into the lower branch. Do not update a second time, even if `(upstream: …)` still lists commits, because a second update duplicates commits.

### Commit selected files or hunks

1. `but diff` — shows file and hunk IDs for uncommitted changes. Do not run plain `but status` first.
2. Use file IDs when whole files belong in the commit; use hunk IDs (`<file-id>:<hunk-id>`) when only part of a file belongs. Omit IDs you don't want committed. For whole files, repo-root paths or a `dir/` prefix work too (`but commit -b feat -m "msg" src/ui/ README.md`; no `./` or globs), so don't build ID lists by piping `but diff` through grep.
3. `but commit -b <branch> -m "<msg>" <id1> <id2>` — the branch is created if it does not exist, so no prior `but branch new` is needed.
4. When the task requires knowing which changes remain, add `--status-after`; otherwise the created-commit result is sufficient.

Edge case: if wanted and unwanted edits are in the same diff hunk, GitButler cannot split that hunk by ID. Only when the task requires keeping part of that hunk uncommitted, temporarily edit the working tree to isolate the wanted lines, commit those IDs, then restore the leftover lines so they remain uncommitted.

### Amend into existing commit

1. `but status -fv` for the target commit ID and file IDs; `but diff` for hunk IDs when only part of a file belongs.
2. `but amend -t <commit-id> <id> <id>` — one command per target commit. For several target commits, chain the amends with `&&` when every target is a change-ID ref; otherwise run them one at a time with `--status-after` to get fresh refs.

### Split an existing commit

For a two-way split, move selected committed files or hunks directly into a new commit:

1. `but diff <source-commit-id>` — read committed file and hunk IDs.
2. `but split <committed-file-or-hunk-id> [<committed-file-or-hunk-id>...] -m "<message>"` — sources may mix files and hunks, but must come from one commit. The command creates the named commit immediately above the source and leaves unselected changes and the original message in the source. No follow-up reword is needed for the new commit.
3. Add `--status-after` only when the next step needs workspace IDs or details not provided by the mutation result.

For more than two replacement commits, repeat `but split ... -m "<message>"` using the remaining source's committed changes, or use the uncommit/recommit workflow below when rebuilding the entire commit:

1. `but status -fv` when you need the source commit, branch name, or placement anchor.
2. `but uncommit <source-commit-id> && but diff` in one shell call exposes the commit's changes and prints the resulting file and hunk IDs.
3. Pick replacement contents from that dirty diff, not from the old committed diff.
4. Determine the requested chronological order from the user's wording, then create the replacement commits oldest-first by chaining `but commit` calls. Do not reverse that order to match `but status`, which displays commits newest-first. `-b <branch>` puts each new commit at the TOP of that branch — if the split commit had commits above it, the replacements now sit above those preserved commits.
5. **If commits from that branch must stay ABOVE the replacements, put the preserved block back on top instead of fighting anchors.** Do not anchor with `--above <top>`/`--below <top>` (sha/`#N` anchors go stale as each insert rewrites history). Move the block together so its internal order stays intact: append `&& but move <preserved-id> [<preserved-id>...] -b <branch>` to the commit chain. Change-ID refs from step 1 stay valid; wait for fresh output first if any preserved ref is sha-based or `#N`-suffixed.
6. Leave unwanted changes uncommitted. Replacements created oldest-first appear newest-first in status — that is correct; do not reorder them. Add `--status-after` to the final mutation when you need to inspect the resulting order.

### Reorder commits

`but status` displays commits newest/top first, while task specs often list history oldest to newest — translate before moving.

1. `but status` once to get commit IDs (use `-fv` only if you also need file details).
2. `but move <source> --below <target-commit>` places source immediately below target in `but status` (older in oldest-to-newest history). `--above` places it immediately above (newer). `but move <source> -b <branch>` moves it to branch top/newest.
3. For an adjacent block, run ONE move, anchored either way: `but move <block-id> <block-id> --below <following-commit-id>` or `but move <block-id> <block-id> --above <preceding-commit-id>`. Pick an anchor outside the block; source order does not matter and the block keeps its internal order. Add `--status-after` when you need to inspect the resulting order; do not move the anchor or block members again.
4. For other reorders, make the smallest set of moves.

### Squash commits

1. `but status` for commit IDs and order.
2. Name the sources positionally and the result/target commit with `-t`: `but squash <source> [<source>...] -t <target> -m "<new message>"`.
3. To collapse an entire branch into one commit, pass just the branch and no `-t`: `but squash <branch> -m "<new message>"`.
4. Multiple independent groups may run in sequence off one status read (targets keep their change-ID refs); prefer newer/top groups first. Take fresh refs only when a ref is sha-based or `#N`-suffixed.
5. Add `--status-after` to the final squash when you need to inspect the resulting history; do not re-verify with a separate status.

### Stack existing branches

To make one existing branch depend on another: `but move <child-branch-name> --above <parent-branch-name>` (use full branch names; commit reordering uses commit IDs). To unstack: `but move <branch-name> --unstack`.

**DO NOT** stack via `uncommit` + `branch delete` + `branch new --above`, and do not use `but undo` to unstack.

### Work in a linked worktree

Use a worktree when the task needs a separate checkout, for example to build or test one branch while other work stays in the main checkout.

1. `but worktree new <branch-name>` checks out that branch in a new directory, creating it if needed (an existing branch must not be in the workspace), and prints that directory. Add `-A <commit>` to start the branch at that commit instead of the workspace base. Worktrees made by other tools must sit outside the repository directory or be listed in `.git/info/exclude`; otherwise a bare `but commit` in the main checkout records them in the branch.
2. Run `but` from that directory, or with `but -C <path>`. There, a bare `but commit -m "<msg>"` or `but diff` takes that worktree's changes, a bare path names a file there, and the commit goes on the worktree's branch. Do the work on that branch, also in a worktree another tool made. `-b <new-name>` is refused there; stack a new branch on it only when the user asks for a separate branch. If `but` says the worktree "is not managed by GitButler", it is archived: `but worktree unarchive <name>`. `but undo` refuses to restore a branch a worktree has checked out, so reverse a mistake there with the inverse command (`uncommit`, `reword`).
3. `but status` from any checkout shows the worktree as a lane of its own, with the same IDs everywhere. `@` always means the main worktree's uncommitted changes; `<worktree>:@` names a linked worktree's changes from anywhere.
4. Push and open PRs as for any branch (`but pr new <branch-name>`). `but pull` also rebases worktree branches. If that leaves a worktree commit `{conflicted}`, the branch name doesn't find it: take its commit ID from `but status` and run `but resolve conflicts <commit-id>`, then `but resolve apply <path>:<N> --commit <commit-id>`.
5. When done, `but worktree remove <name>` deletes the directory and keeps the branch. It refuses while the worktree has uncommitted changes; `-f` discards them.

### Create or manage pull requests

`but pr new <branch-name>` pushes the selected branch and its ancestors, then creates the PR in one step — no prior `but push`. Provide `-F -` with a heredoc or `-F <file>` (first line is the title), `-t`, or `-m` with real newlines (zsh/bash: `-m $'Title\n\nBody'`) so no editor opens. Do not write the message to a file inside the repository: the next `but commit` would commit it. If forge auth is missing, ask the user to run `but config forge auth` in a terminal; it refuses non-interactive runs.

If you do create a PR for a stacked branch, use `but pr` — not `gh pr create` (only `but pr` sets PR bases and stack metadata; `gh pr create` breaks that). To publish a whole stack: `but pr new <top-branch-name> -t`. Manage with `but pr auto-merge|set-draft|set-ready <selector>`; selectors can be a branch name, current branch/stack CLI ID, or numeric review ID. See `but skill reference` for details.

When `but pr new` prints a `<pr-created>` line, copy that line into the text of your reply. It tells the app running you that this session opened the PR.

### Dependency conflict with another branch

Changes that build on another branch's commits cannot land on an independent branch. `but commit` and `but amend` fail atomically ("Cannot commit: N changes could not be applied"), naming the branch and commit each rejected change depends on — nothing is committed and no `-b` branch is created.

Stack only when your change builds on that branch's code. A refusal can also mean your hunk only borders lines that branch changed (a doc line, a lockfile): then leave one unchanged line between your edit and theirs, or keep that hunk uncommitted until that branch lands, and commit the rest. Never drop or rewrite real work to get past a refusal; ask the user.

When there is a single dependency branch, the error's Hint gives the exact recovery command: `but move <your-branch> --above <dependency-branch>` to stack an existing branch on its dependency, or `but branch new <name> --above <dependency-branch>` when the target branch didn't exist yet. Run it, then retry the original command. When the error names dependencies without a Hint (several dependency branches, or the dependency is on the target branch itself), run `but status -fv` to see where the dependent commits live before choosing a placement.

If that recovery command fails, do NOT try `uncommit`, `squash`, or `undo` as a workaround — re-run `but status -fv` to confirm both branches exist and are applied, then retry with exact branch names.

### Resolve conflicted commits (after pull, move, or reorder)

**NEVER use `git add`, `git commit`, `git checkout --theirs/--ours`, or any git write command during resolution.** Only `but resolve` commands plus direct file edits. The one exception is a conflicted submodule in edit mode, below.

Conflicts do not interrupt operations in GitButler: a rebase always completes, and commits that conflicted are marked `{conflicted}` in `but status`. Find them from the warning that history-editing commands (`move`, `discard`, …) print, from the `but pull` summary, or from `but status`. Resolve them one conflict at a time, without entering any mode. Work through one branch at a time, oldest commit first — the loop is:

1. `but resolve conflicts <branch>` — shows the conflicts of that branch's oldest conflicted commit, numbered per file. Each conflict has three sections: `ours` (the new base the commit was rebased onto), `base` (common ancestor), and `theirs` (the commit's own version). Without a branch it picks the first conflicted branch and tells you which; the output also says when other conflicted commits exist.
2. Apply one resolution per command:
   - Merged/mixed content (the common case — combine the intent of both sides): pipe it to `but resolve apply <path>:<N>` via stdin (heredoc) or pass `--file <f>`. The content replaces the whole conflicted region; never include conflict markers.
   - Take a side entirely: `but resolve apply <path>:<N> --ours` or `--theirs` (bare `<path>` applies the side to all conflicts in that file).
   - Delegate one conflict to the configured AI model: `but resolve apply <path>:<N> --ai` — prefer your own merged content when you have the context.
   - `apply` targets the same default commit as `conflicts`; when more than one branch is conflicted, pin it with `--commit <branch>`.
3. Go back to step 1 until it reports no conflicts. Commit ids are not stable here — every apply rewrites the commit — but branch names are, so address everything by branch and never bookkeep commit ids. Partial progress is fine: a commit with unresolved conflicts left stays `{conflicted}` with exactly those conflicts.

A wrong resolution is reverted with `but undo`.

`but resolve <commit-id> --ai` resolves a whole commit with the configured AI model in one shot, and `but resolve --ai` without a commit resolves every conflicted commit, oldest first — acceptable as a fallback, but you usually have better context than that model does, so prefer resolving the conflicts yourself with `apply`.

**Use edit mode only when you must run code against the resolution** (build/tests at that commit). Edit mode needs `gitbutler/workspace`: while a single branch is checked out, `but resolve <commit-id>` refuses, so use `apply`.

1. `but resolve <commit-id>` — enters resolution mode and prints the conflict regions.
2. **Edit the files** to remove every conflict marker — `<<<<<<<`, `|||||||` (the common-ancestor section), `=======` and `>>>>>>>` — and keep the correct content. Do NOT skip this; do NOT use `but amend` on conflicted commits.
   - A conflicted submodule has no markers and `finish` refuses until you select its side: see Conflict Resolution Mode in `but skill concepts`.
3. `but resolve finish` reports leftover markers, surviving uncommitted changes, every remaining conflicted commit, and the exact current `but resolve <id>` command. Add `--status-after` to the finish you expect to clear the last conflict only when the task needs the complete resulting workspace. When it says no conflicted commits remain, stop; do not run a verification status. Cancel instead with `but resolve cancel --force`, which drops your resolution edits; without `--force` it refuses once the conflict markers are checked out.
4. Repeat for remaining conflicted commits, oldest first — finishing a lower commit rebases the ones above it.

### Conflicts in uncommitted files

`but status` marks uncommitted files with unresolved merge conflicts `{conflicted}`; they are excluded from committable changes and outside `but resolve` mode. Edit the file to the wanted contents (or delete it), then `but resolve <path>...` to mark it resolved with that state; it then shows as an ordinary uncommitted change.

## Git-to-But Map

| git | but |
|---|---|
| `git status` | `but status` for branch/stack/commit overview; `but status -fv` for file details; `but diff` for selected dirty changes |
| `git add` + `git commit` | `but commit -b <branch> -m ... <ids>` |
| `git checkout -b` + commit | `but commit -b <new-branch> -m ... <ids>` |
| `git push` | `but push <branch-name>` |
| `git rebase -i` | `but move`, `but squash`, `but reword` |
| `git rebase --onto` | `but move <branch> --above <new-base>` |
| `git checkout -- <file>` / `git restore` | `but discard <path>`, or a hunk ID from `but diff` |
| `git cherry-pick` | `but pick` |
| `git fetch` | `but pull --check` (fetches; changes no branch) |
| `git pull` on a branch others pushed to | `but branch update <branch>`, then `but pull` |
| `git worktree add -b <branch>` | `but worktree new <branch>` |
| `git worktree remove` | `but worktree remove <name>` |
| `gh pr create` | `but pr new <branch-name> -m "Title..."` |

## Notes

- `but` needs no setup in a Git repository. `HEAD` is either one checked-out branch, marked `[HEAD]` in `but status`, or `gitbutler/workspace`, which merges every applied stack. Applying a second stack, or applying a branch while the target branch is checked out, switches to `gitbutler/workspace`; unapplying back to one stack checks that branch out again. Commands and IDs are the same either way. It does need a local branch tracking the remote's default branch: if commands fail with "No target branch is configured", run `git branch --track main origin/main` (with the default branch's name), not `--set-upstream-to` on your own branch, which makes that branch the target.
- `but undo` restores the whole snapshot, so uncommitted edits made after the undone operation are lost (`but redo` brings them back). Commit them first.
- Read-only git inspection (`git log`, `git blame`, `git show --stat`) is allowed.
- If `but` prints an `AGENT ACTION REQUIRED` skill warning, run the suggested command once, then reload/use the GitButler skill. If it repeats, report it instead of retrying.
- For command syntax and flags: `but skill reference`
- For workspace model: `but skill concepts`
- For workflow examples: `but skill examples`
