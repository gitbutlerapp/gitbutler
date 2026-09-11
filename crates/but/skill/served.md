# GitButler CLI Skill

`but` is the version-control interface. GitButler keeps several branches applied in one working directory and assigns changes to them, so anything that moves refs, the index or the working tree (`add`, `commit`, `checkout`, `reset`, `restore`, `rebase`, `merge`, `stash`, `cherry-pick`, `fetch`, `pull`, `push`) goes through `but`; run raw they bypass its bookkeeping, and the workspace they leave behind is not the one `but` describes. Read-only git (`log`, `blame`, `show`) is fine. When the user names a git write command, run the `but` equivalent.

Applied branches share the current workspace. Being "on" one means keeping it applied; leaving the workspace for a plain Git checkout is a step only when the user asks for it.

## The loop

One inspection read prints the IDs, mutations take those IDs, and each mutation prints its result. Pick the narrowest first read for the task:

```bash
but diff            # uncommitted files and hunks with IDs: committing selected changes
but status          # branches, stacks, commit order, conflicts
but status -fv      # plus the files in each commit: amend, split, uncommit
but diff <commit>   # one commit's files and hunks with IDs
but show <id>       # one branch or commit in detail
```

Several mutations can run off one read, chained with `&&`, as long as none needs an ID the previous one issues. A mutation's output is the result: `Created commit vms on new branch 'feat-a'` means the commit exists on that branch, and the next command is the next step of the task. Add `--status-after` to a mutation when the following step needs the resulting workspace (new IDs, the new order); otherwise it is not needed.

## IDs

After the graph glyphs, the first token on a `but status` line is that line's ID. A `but diff` header reads `<file>:<hunk> path`: the whole token is the hunk's ID, the part before the colon is the file's. Copy IDs from current output. The kinds:

- `uvw` an uncommitted file; `uvw:2e4` an uncommitted hunk. The part after the colon is the hunk's ID, not a line range, and is never passed on its own.
- `mzm:uvw` a committed file (listed under each commit by `but status -fv`); `mzm:uvw:2e4` a committed hunk (from `but diff <commit>`). `uncommit`, `squash -t` and `move` take them as sources to pull one file's or hunk's changes out of a commit.
- `mzm` a commit. It is a change-ID prefix and survives `amend`, `squash`, `move`, `uncommit` and `reword`. A commit without a change ID shows a sha prefix instead, and `#N` suffixes tell duplicates apart; both go stale after any history edit, so take them from fresh output. The `(sha …)` on verbose lines is informational.
- A branch: use its full name in mutations. Short branch IDs are valid only for the read that printed them.
- `@` the uncommitted area.

IDs are positional and space-separated; `uvw,qyo` is one unknown ID. `but diff` takes at most one target. File and hunk IDs from one `but diff` stay usable across a chain of commits; if one stops resolving, re-read `but diff` and continue.

## Rules

- Pass on the command line everything a terminal would prompt for: `-m` (or `--no-message`) on `commit` and `reword`, and on `squash` when the sources are commits or a branch (squashing files or hunks into a commit keeps the target's message and takes no message flag); `-m`, `-F <file>` or `-t` on `pr new`; `--yes` on `merge`; a branch name on `push`. Without them, a terminal opens an editor or a prompt; an agent run keeps what the command composed (an empty message for `commit`, the joined source messages for `squash`), `reword`, `pr new` and `merge` refuse, and `push` pushes every stack.
- `but commit` needs a target when more than one stack is applied: `-b <branch>` creates the branch if it does not exist, `--above`/`--below <commit-or-branch>` place the commit in history. Branches stacked together are one stack, and an untargeted commit lands on its top branch, so pass `-b` whenever the branch matters.
- Branches marked `(merged upstream)` have landed. `but pull` removes them; `push` and history edits refuse them. Start new work on another branch.
- Every successful mutation is recorded; `but undo` restores the snapshot taken before the last recorded one, working tree included, so uncommitted edits made after that point are lost and a command that failed recorded nothing. The `Undid …: <operation>` line says what was undone.

## Finishing

The task is done when the printed results cover what was asked. A history edit that cannot carry a change cleanly marks that commit `[conflict]` in its output and `{conflicted}` in status instead of altering it silently; a result without that marker applied every commit's changes and kept its message, and re-deriving that with git repeats what the output stated. When the task needs a fact the result does not state (the resulting order, what is left uncommitted), take it once with the narrowest read; a fact the mutation's own output states does not need a second command.

Pending changes are what `but diff` shows. Leaving work uncommitted or unstaged in this workspace means leaving it pending. Git's index does not queue it for a future `but commit`; select that commit's contents by ID, since omitting IDs can include pending work. Discarding and recreating files to change Git's index display does not change their `but` state. Use Git's view for an explicitly requested plain checkout.

Leave the working tree as the task described it. Whatever your own verification created (build output, caches, temporary copies) is yours to remove; check for it after your last own command, not before, and a leftover you noticed is not something to hand to the user. Pre-existing files, the user's uncommitted work and the deliverables are not yours to touch.

## When a command fails

The error names the fix, often on a `Hint:` line. Apply it once; the same call again returns the same error, and git does not repair a `but` state.

- `Could not find uncommitted change: 'x'`: the ID is a bare hunk suffix (a hunk is `file:hunk`), comma-joined, or from an earlier read. Re-read `but diff` and copy whole, space-separated IDs.
- `Unclear where to commit. Found more than one stack`: add `-b <branch>`, `--above` or `--below`.
- `Cannot commit: N changes could not be applied … depends on <branch> (<commit>)`: the change builds on another branch's commits, and nothing was committed (an all-changes commit is all-or-nothing). Stack your branch on the dependency and retry: `but move <your-branch> --above <that-branch>` when your branch exists, `but branch new <your-branch> --above <that-branch>` when it does not; or commit onto that branch instead. If the retry is rejected again, or several branches are named, `but status -fv` shows which stack holds those commits; place your branch above the top branch of that stack.
- `the following required arguments were not provided: --target`: `amend` takes `-t <commit-or-branch>`.
- `No conflicted commits found`, or a branch `has no conflicted commits`: resolution is complete.
- `Setup required`: you are not in a GitButler project. Check the directory first (`but -C <repo> status`); if the repository has never been set up, run `but setup` once in its root, then continue.
- `unexpected argument`: the flag belongs to git or `gh`, or the installed `but` predates this guide; the `Usage:` line in the error shows what this binary accepts. `but <cmd> --help` shows the command's flags and examples; `but skill reference` shows every command's.

## Recipes

### Commit selected files or hunks

```bash
but diff
but commit -b <branch> -m "<msg>" <id> <id>
```

`-b` creates the branch when it does not exist. Use file IDs for whole files and `<file>:<hunk>` for part of a file. Omitting the IDs commits everything uncommitted, including changes that are not yours, so pass IDs whenever the tree holds other work; with nothing uncommitted, the omitted form creates an empty commit. Several commits from one diff chain with `&&` and stack oldest-first in the order written. When wanted and unwanted lines share one hunk, edit the file so only the wanted lines remain, commit, then restore the rest.

### Amend into an existing commit

```bash
but status -fv
but amend -t <commit> <id> <id>
```

One `amend` per target commit; a branch target means its newest commit. Chain the amends when every target is a change-ID ref.

### Split a commit

Two-way: read `but diff <commit>`, then `but split <commit>:<file> [<commit>:<file>:<hunk> …]` moves the sources into a new commit directly above the source and leaves the rest. The result names it (`… to new commit <id> above commit <src>`); it has no message, so `but reword <id> -m "<msg>"` follows directly.

More than two, or with messages chosen up front:

```bash
but status -fv                                   # source commit, branch, what sits above it
but uncommit <commit> && but diff                # the commit's changes as uncommitted hunks
but commit -b <branch> -m "<first>" <id> … && but commit -b <branch> -m "<second>" <id> …
```

Pick the replacement contents from that dirty diff. Create them oldest-first; `but status` lists commits newest-first, so the replacements appear in reverse there, which is correct. `-b` puts each new commit at the top of the branch. If commits from that branch must stay above the replacements, append `&& but move <preserved-id> [<preserved-id> …] -b <branch>` to the same chain; the block keeps its internal order and the change-ID refs from the first read stay valid. Leave unwanted changes uncommitted.

### Reorder commits

`but status` shows newest first; a task that lists history oldest to newest reads the other way. Then one move per block:

```bash
but move <id> [<id> …] --below <commit>    # or --above <commit>; anchor outside the block
but move <id> -b <branch>                  # to the top of a branch
```

Source order does not matter and the block keeps its internal order. The move's output is the result: each moved commit keeps its message and changes unless the output marks it `[conflict]`, so comparing patches afterwards re-establishes what the output already stated. The one fact a move leaves unstated is the new order; `--status-after` on the last move shows it when the task asks for it.

### Squash commits

```bash
but squash <src> [<src> …] -t <target> -m "<msg>"   # commits into an existing commit
but squash <branch> -m "<msg>"                       # a whole branch into one commit
```

`Squashed <src> into <target>` is the result; the target's changes and the sources' are combined unless it is marked `[conflict]`. Several independent squashes chain from one status read as long as every ID is a change-ID ref.

### Update the workspace from main

`but pull` fetches the target branch and rebases every applied branch onto it in one step, and reports which commits conflicted; resolving those is part of the update (next section). The `(upstream: <remote>/<branch>)` line in `but status` names the target and shows its last fetched state, so a `main` that git shows as ahead of it is exactly what `but pull` applies; retargeting is never the fix. Uncommitted changes ride along; one that conflicts with the update is not refused but marked `{conflicted}` in status with conflict markers in the file. `but pull --check` previews without updating, for when the user asks for a preview.

`but pull` integrates the target only. Commits pushed to a branch's own remote counterpart are listed by `but status -u` and integrated by `but branch update <branch>`, which rebases the local commits on top of the remote ones by default (`--dry-run` previews). `but pull` fetches; `but branch update` does not, it integrates what the last fetch brought. When both the target and a branch need updating, `but pull` first, then `but branch update <branch>`; for a branch alone, `but pull --check` fetches without integrating.

### Resolve conflicted commits

Conflicts do not interrupt an operation: the rebase completes and the affected commits are marked `{conflicted}`. Resolve them from the workspace, one branch at a time, oldest commit first, since finishing a lower commit rebases the ones above it:

```bash
but resolve conflicts <branch>                   # the branch's oldest conflicted commit, conflicts numbered per file
but resolve apply <path>:<N> --ours              # the new base's side; --theirs is the commit's own side
but resolve apply <path>:<N> --file merged.txt   # or pipe merged content on stdin; no conflict markers
```

`apply` targets the same default commit as `conflicts`; with several conflicted branches pin it with `--commit <branch>`. Commit IDs change on every apply, branch names do not, so address everything by branch. Each `apply` reports what is left: `N conflicts remaining` (keep going in this commit), `All conflicts in this commit are resolved` with `Other commits are still conflicted` (run `but resolve conflicts` for the next one), or `All conflicts in this commit are resolved` alone, which means resolution is finished and no further `conflicts` call is needed. `but undo` reverts a wrong resolution. `but resolve apply … --ai` and `but resolve <commit> --ai` delegate the merge to the configured model.

To build or run tests at a conflicted commit, use resolution mode instead: `but resolve <commit>` checks it out and prints the conflict regions; edit the files until no `<<<<<<<`, `|||||||`, `=======` or `>>>>>>>` remains; `but resolve finish` (or `cancel`) returns to the workspace and names the next conflicted commit, if any.

Uncommitted files with merge conflicts show `{conflicted}` in `but status`; edit the file to its wanted contents, then `but resolve <path>` marks it resolved and committable.

### Stack branches

`but move <child-branch> --above <parent-branch>` makes an existing branch depend on another; `but move <branch> --unstack` tears it off. Full branch names on both sides. Stacking is a move, not a rebuild: `branch delete` discards the branch's commits, so deleting and recreating a branch to restack it loses them.

### Push and pull requests

`but push <top-branch>` pushes that branch and the branches below it in its stack; one push per stack. `but pr new <branch> -m "<title>"` pushes first and creates the review; for a body use `-F <file>` (first line is the title, the rest the body; a real path, not `-`) or `-t` for the default text; `but pr new <top-branch> -t` publishes a whole stack. Use `but pr`, not `gh pr create`, for stacked branches, since only `but pr` sets the PR bases and stack metadata. `but pr auto-merge|set-draft|set-ready <branch>` manage existing reviews. If forge authentication is missing, `but config forge auth`.

## From git habits

| git | but |
|---|---|
| `git stash` | there is none: `but commit -b <branch> -m "wip" <ids>`, later `but uncommit <commit>` |
| `git checkout -- <file>` / `git restore` | `but discard <id>` |
| `git rebase -i` | `but move`, `but squash`, `but reword` |
| `git rebase --onto <base>` | `but move <branch> --above <base>` |
| `git fetch` / `git pull` | `but pull` fetches and updates from the target branch; `but branch update <branch>` integrates a branch's own fetched remote |
| `git cherry-pick` | `but pick <sha> -b <branch>` |
| `gh pr create` | `but pr new <branch> -m "…"` |

## More

- `but skill reference`: every command with its arguments and flags.
- `but skill concepts`: the workspace model, applied and unapplied branches, IDs in depth.
- `but <cmd> --help`: examples for one command.
