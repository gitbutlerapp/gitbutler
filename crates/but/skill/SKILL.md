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
but commit -b <branch> -m "<msg>" <id> <id>
```

`but commit -b <branch> ...` creates the branch when it does not exist and prints the created commit. Do not run a separate `but branch new`, status command, or verification diff unless the task needs workspace information that the result does not provide.

## IDs

The first token on each `but diff` / `but status` line is that line's ID — pass it to commands as-is; never hardcode or invent IDs. IDs may be a single character when unambiguous; copy them exactly from command output.

- Changes and sources are **positional, space-separated** IDs (`but commit -b feat -m "msg" qs:5 uo`). A hunk ID is written `<file-id>:<hunk-id>` (e.g. `qs:5`, copied from `but diff`) — the part after the colon is the hunk's ID, **not** a line range (`qs:16-40` is invalid). Do not invent flags like `--changes` / `--hunk` / `--ids`, pass a line range, or comma-separate IDs — `nk,pn` is parsed as one ID and fails.
- `but diff` is the exception: it accepts at most **one** target. Bare `but diff` shows all uncommitted files; inspect committed files or other entities one target at a time — never `but diff <id> <id>`.
- A committed file is `<commit-id>:<file-id>` (e.g. `uyr:n`, shown under each commit). `zz` means the uncommitted area.
- Commit IDs are stable change IDs that survive history edits (`amend`, `squash`, `move`, `uncommit`, `reword`). Commits without a change ID (e.g. upstream-only) lead with a sha prefix instead, and `#N`-suffixed refs disambiguate duplicates — both go stale after history edits, and a stale sha can silently resolve to the wrong commit. The `(sha …)` on verbose commit lines is informational — do not pass it to commands.
- File/hunk IDs copied from one diff read generally remain usable across chained commits; branch IDs are stable. If an ID stops resolving, re-read `but status`/`but diff` and retry.

**Chaining:** mutation output is concise by default, so you may chain mutations with `&&` off one inspection read. Add `--status-after` only when the next step needs workspace IDs or details that the mutation result does not provide. Chained `but commit` calls stack in the order written — the first is oldest, each later one goes on top. History edits may run in sequence when every commit ref involved is a change-ID ref; run them one at a time with `--status-after` when a ref is sha-based or `#N`-suffixed, or when the next command needs freshly issued IDs. Chaining `but uncommit <id> && but diff` is safe because bare `diff` needs no ID from the uncommit output.

## Non-Negotiable Rules

1. Use `but` for all write operations. Never run `git add`, `git commit`, `git push`, `git checkout`, `git merge`, `git rebase`, `git stash`, or `git cherry-pick`. If the user says a `git` write command, translate it to `but` and run that. Exceptions: `git add -- <path>` to mark a conflicted uncommitted file resolved (see "Conflicts in uncommitted files"), and a worktree-local Git commit when `but commit` reports that linked worktrees are unsupported. Never run `but setup` from a linked worktree.
2. Mutation commands print their result without appending workspace status. Add `--status-after` only when the next step needs resulting workspace IDs or details; otherwise trust the mutation result and do not run a verification status/diff.
3. Branches marked `(merged upstream)` have landed; run `but pull` to remove them, or start new work on another branch. `push` and mutations (`commit`, `amend`, `squash`, `uncommit`, `reword`, `move`) refuse landed branches and commits, `absorb` skips them with a notice, and `commit` skips them when picking a default target.
4. In non-interactive CLI workflows, do not narrate progress between routine commands. Execute the needed `but` commands and give a concise final summary.
5. Prefer this skill and `references/reference.md` over exploratory help calls. Use `<command> --help` when required syntax is missing or a command fails; use top-level help only when you genuinely need to discover an undocumented command.

## Command Patterns

- Commit: `but commit -b <branch> -m "<msg>" <id> <id>` — `-b <branch>` creates the branch if it does not exist
- Commit everything uncommitted: `but commit -b <branch> -m "<msg>"` (omit the IDs)
- Several commits from one diff: chain `but commit` calls with `&&` (commits stack oldest-first)
- Commit at a specific history position: `--above <commit-or-branch>` or `--below <commit-or-branch>` instead of `-b`
- Only one targeting flag (`-b` / `--above` / `--below`) per command. Targeting is **required** when more than one **stack** is applied; without it `but commit` fails with "Unclear where to commit. Found more than one stack". Several branches stacked together count as one stack — an untargeted commit then silently lands on the stack's top branch, so pass `-b` whenever the branch matters.
- Always pass `-m "<msg>"` (or `--no-message`) to `but commit`, and to `but squash` whenever its sources are commits or branches unless the target is `zz` — those compose a new message, and without a flag an editor opens and blocks. Squash sources that are uncommitted or committed files reuse the target's message and need no flag; squashing into `zz` rejects message flags outright.
- Amend: `but amend -t <commit-or-branch> <file-or-hunk-id> <file-or-hunk-id>` — a branch target resolves to its newest commit
- Uncommit: `but uncommit <commit-id>` (whole commit), `but uncommit <branch>` (all commits and remove the branch), or `but uncommit <commit-id>:<file-id>` (one committed file); multiple committed-file sources in one call must come from one commit
- Insert empty commit: `but commit --empty -b <branch> -m "<msg>"`
- Squash commits: `but squash <source-commit-id> [<source-commit-id>...] -t <target-commit-id> -m "<msg>"`
- Squash a whole branch into one commit: `but squash <branch> -m "<msg>"` (no `-t`)
- Uncommit and remove a branch: `but uncommit <branch>`
- Reorder commits: `but move <commit-id> --below <commit-id>` (`--above` for the other direction; **commit IDs**, not branch names)
- Reorder a block: `but move <commit-id> <commit-id> --below <following-commit-id>` or `--above <preceding-commit-id>` (both anchors accept multiple space-separated sources)
- Move commit to branch top: `but move <commit-id> -b <branch>`
- Stack branches: `but move <branch> --above <target-branch>` (**branch names or branch CLI IDs**)
- Tear off a branch: `but move <branch> --unstack`
- Discard: `but discard <id> [<id>...]` — accepts branches, commits, committed files, uncommitted files/hunks, or `zz` for all uncommitted changes
- Push: `but push <top-branch>` — pushes the selected branch and its ancestors; to update a stack, select its top branch once and never loop. Bare `but push` pushes all unpushed work when run non-interactively — one push per stack (its topmost unpushed branch, ancestors included), so output has one entry per stack, not per branch. It exits non-zero if any stack failed; stacks that already pushed stay pushed, and rerunning after fixing the failure is safe (up-to-date stacks are skipped)
- Pull (update workspace from the target): `but pull` — the output reports the result; `but pull --check` previews without updating when a preview is actually needed
- Create PR: `but pr new <branch-id> [-m "Title..."] [-F pr_message.txt] [-t] [--draft]` — auto-pushes first; do not run `but push` before it

## Task Recipes

### Update workspace from main

For "get latest from main", "update/sync this workspace", "rebase onto main", or "pull main":

1. `but pull` — one command; no preflight needed. Its output reports the resulting state, it refuses safely when uncommitted changes conflict, and `but undo` reverts it.
2. If commits come back conflicted, resolve them oldest-first following the printed instructions: `but resolve <commit>`, edit the files, then `but resolve finish`. Its result gives the current ID of the next conflict. Add `--status-after` to the finish you expect to clear the last conflict only when the task needs the complete resulting workspace. When it says no conflicted commits remain, stop; do not run a verification status. Finishing a lower commit rebases the ones above it, so always work bottom-up.

`but pull --check` answers "would this conflict?" without updating. Do not use it as a routine
preflight; use it when the user asks for a preview, repository policy requires one, or other agents'
branches may move.

Rebasing applied branches onto the latest target IS `but pull` — never `move`, `config target`, `unapply`, or raw `git pull`/`git rebase`. The base shown in status is the last FETCHED state: when `git log` shows `main` (local or remote) ahead of it, that is exactly the update `but pull` fetches and applies — the target setting is not stale and repointing it is never the fix. Pull carries uncommitted changes along, and its output reports the resulting state. If it refuses because uncommitted changes conflict, park them: `but commit -b <branch> -m "wip" <ids>`, pull again, then `but uncommit` the parked commit (there is no stash; do not hand-revert files).

### Commit selected files or hunks

1. `but diff` — shows file and hunk IDs for uncommitted changes. Do not run plain `but status` first.
2. Use file IDs when whole files belong in the commit; use hunk IDs (`<file-id>:<hunk-id>`) when only part of a file belongs. Omit IDs you don't want committed.
3. `but commit -b <branch> -m "<msg>" <id1> <id2>` — the branch is created if it does not exist, so no prior `but branch new` is needed.
4. When the task requires knowing which changes remain, add `--status-after`; otherwise the created-commit result is sufficient.

Edge case: if wanted and unwanted edits are in the same diff hunk, GitButler cannot split that hunk by ID. Only when the task requires keeping part of that hunk uncommitted, temporarily edit the working tree to isolate the wanted lines, commit those IDs, then restore the leftover lines so they remain uncommitted.

### Amend into existing commit

1. `but status -fv` (or `but show <branch-id>`) — locate file/hunk IDs and target commit IDs.
2. `but amend -t <commit-id> <id> <id>` — one command per target commit. For several target commits, chain the amends with `&&` when every target is a change-ID ref; otherwise run them one at a time with `--status-after` to get fresh refs.

### Split an existing commit

Use this when an existing commit should be replaced by selected smaller commits.

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

To make one existing branch depend on another: `but move <child-branch> --above <parent-branch>` (branch **names** or branch CLI IDs — commit reordering uses commit IDs). To unstack: `but move <branch> --unstack`.

**DO NOT** stack via `uncommit` + `branch delete` + `branch new -a` (git branch names persist after delete and it loses work), and do not use `but undo` to unstack.

### Create or manage pull requests

`but pr new <branch-id>` pushes the selected branch and its ancestors, then creates the PR in one step — no prior `but push`. Provide `-F pr_message.txt`, `-t`, or `-m` with real newlines (zsh/bash: `-m $'Title\n\nBody'`) so no editor opens. If forge auth is missing, run `but config forge auth`.

For stacked branches `but pr` is mandatory (it sets PR bases and stack metadata; `gh pr create` breaks that). To publish a whole stack: `but pr new <top-branch-id> -t`. Manage with `but pr auto-merge|set-draft|set-ready <selector>`. See `references/reference.md` for details.

### Dependency conflict with another branch

Changes that build on another branch's commits cannot land on an independent branch. `but commit` and `but amend` fail atomically ("Cannot commit: N changes could not be applied"), naming the branch and commit each rejected change depends on — nothing is committed and no `-b` branch is created.

When there is a single dependency branch, the error's Hint gives the exact recovery command: `but move <your-branch> --above <dependency-branch>` to stack an existing branch on its dependency, or `but branch new <name> --anchor <dependency-branch>` when the target branch didn't exist yet. Run it, then retry the original command. When the error names dependencies without a Hint (several dependency branches, or the dependency is on the target branch itself), run `but status -fv` to see where the dependent commits live before choosing a placement.

If that recovery command fails, do NOT try `uncommit`, `squash`, or `undo` as a workaround — re-run `but status -fv` to confirm both branches exist and are applied, then retry with exact branch names.

### Resolve conflicted commits (after pull, move, or reorder)

**NEVER use `git add`, `git commit`, `git checkout --theirs/--ours`, or any git write command during resolution.** Only `but resolve` commands plus direct file edits.

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

**Use edit mode only when you must run code against the resolution** (build/tests at that commit):

1. `but resolve <commit-id>` — enters resolution mode and prints the conflict regions.
2. **Edit the files** to remove every conflict marker — `<<<<<<<`, `|||||||` (the common-ancestor section), `=======` and `>>>>>>>` — and keep the correct content. Do NOT skip this; do NOT use `but amend` on conflicted commits.
3. `but resolve finish` reports leftover markers, surviving uncommitted changes, every remaining conflicted commit, and the exact current `but resolve <id>` command. Add `--status-after` to the finish you expect to clear the last conflict only when the task needs the complete resulting workspace. When it says no conflicted commits remain, stop; do not run a verification status. Cancel instead with `but resolve cancel`.
4. Repeat for remaining conflicted commits, oldest first — finishing a lower commit rebases the ones above it.

### Conflicts in uncommitted files

`but status` marks uncommitted files with unresolved merge conflicts `{conflicted}`; they are excluded from committable changes and outside `but resolve` mode. Choose the desired contents or delete the file, then `git add -- <path>` to mark it resolved (the one permitted `git add`).

## Git-to-But Map

| git | but |
|---|---|
| `git status` | `but status` for branch/stack/commit overview; `but status -fv` for file/hunk details; `but diff` for selected dirty changes |
| `git add` + `git commit` | `but commit -b <branch> -m ... <ids>` |
| `git checkout -b` + commit | `but commit -b <new-branch> -m ... <ids>` |
| `git push` | `but push <branch-name>` |
| `git rebase -i` | `but move`, `but squash`, `but reword` |
| `git rebase --onto` | `but move <branch> --above <new-base>` |
| `git checkout -- <file>` / `git restore` | `but discard <id>` |
| `git cherry-pick` | `but pick` |
| `gh pr create` | `but pr new <branch-id> -m "Title..."` |

## Notes

- Read-only git inspection (`git log`, `git blame`, `git show --stat`) is allowed.
- If `but` prints an `AGENT ACTION REQUIRED` skill warning, run the suggested command once, then reload/use the GitButler skill. If it repeats, report it instead of retrying.
- For command syntax and flags: `references/reference.md`
- For workspace model: `references/concepts.md`
- For workflow examples: `references/examples.md`
