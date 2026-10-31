# GitButler CLI Key Concepts

Deep dive into GitButler's conceptual model and philosophy.

## The Workspace Model

### GitButler: Parallel Stacks

```
workspace (gitbutler/workspace)
  ├─ feature-a (applied, merged into workspace)
  ├─ feature-b (applied, merged into workspace)
  └─ feature-c (unapplied, not in workspace)
```

- Work on MULTIPLE branches simultaneously
- No context switching - all applied branches merged in working directory
- Changes are ASSIGNED to branches, not isolated by checkout

### Single-Branch Mode

GitButler works in any Git repository, on the branch that is checked out. There is no setup
command. A repository starts in single-branch mode: `HEAD` is the stack's top branch, as in
plain Git, and there is no `gitbutler/workspace` branch:

```
╭┄ @ [uncommitted] (no changes)
┊
┊╭┄ se [settings-page] [HEAD]
┊●   wtnt Add settings page
┊│
┊├┄ et [settings-model]
┊●   xyow Add settings model
├╯
┊
┴ a0498e6 (common base, main, origin/main) 2026-10-26 Initial commit
```

- With the target branch checked out (for example a fresh clone of `main`),
  `but commit -b <name>` creates that branch and checks it out. A bare `but commit` there
  checks out a new branch with a generated name.
- A branch stacked on the checked-out one (`--above`) becomes the new `HEAD`; the repository
  stays in single-branch mode.
- A second independent stack (`but apply`, or `but commit -b`/`but branch new` for a new
  branch while a feature branch is checked out) switches `HEAD` to `gitbutler/workspace`.
- `but apply` while the target branch is checked out also switches to `gitbutler/workspace`,
  even for a single stack.
- Unapplying, discarding, or deleting branches down to one stack checks that stack's branch out
  again. Other ways of dropping to one stack, such as squashing a whole branch into `@`, can
  leave `HEAD` on `gitbutler/workspace`.

Commands, IDs, and recipes are the same in both modes, so you do not need to track which mode
the repository is in. The exception is edit mode (`but resolve <commit-id>`), which needs `gitbutler/workspace`. `[HEAD]` in `but status` marks the checked-out branch in single-branch mode.

### Key Implications

1. **No `git checkout`**: You don't switch between branches. All applied branches exist simultaneously in your workspace.

2. **The `gitbutler/workspace` branch**: With two or more stacks applied, or after `but apply` from the target branch, `HEAD` is this branch, a merge commit containing all applied stacks. Don't interact with it directly - use `but` commands.

3. **Applied vs Unapplied**: Control which branches are active:
   - Applied branches: In your working directory
   - Unapplied branches: Exist but not active
   - Use `but apply`/`but unapply` to control

## CLI IDs: Short Identifiers

Every object gets a short, human-readable CLI ID shown in `but status` or `but diff`. IDs are generated per-session and are unique across all entity types (no two objects share an ID) — always read them from current command output.

```
Commits:          1, ton, mpq#0       (short change-ID prefix when the commit has one, sha prefix otherwise;
                                   a #N suffix disambiguates commits sharing a change ID)
Branches:         at, se, et          (unique 2–3 char substring of the branch name that doesn't look like hex,
                                   e.g. "at" from "feature-x";
                                   falls back to auto-generated ID if no unique substring exists)
Files:            uvw, qyo            (derived from the file path, long enough to be unique)
Hunks:            uvw:2e4, uvw:e2c    (<file-id>:<hunk-id>; uncommitted hunks shown by bare `but diff`)
Committed files:  mzm:uvw             (<commit-id>:<file-id>, shown under each commit in `but status -fv`)
Committed hunks:  mzm:uvw:2e4         (<commit-id>:<file-id>:<hunk-id>, shown by `but diff <commit-id>`)
```

**ID lengths:** When an agent is detected, shortened change-ID, file, and hunk prefixes have a four-character minimum; SHA prefixes and branch/worktree IDs can be shorter. Mutation results print shorter commit IDs (`Created commit ppy`), which can be ambiguous; take IDs from `but status` or `but diff`.

**Reading status output:** the first token on each line is that line's ID. Verbose commit lines append an informational `(sha …)` after the timestamp — it changes on every amend; do not pass it to commands.

**Stability:** Branch short IDs identify branches only within the workspace snapshot that produced them; branch names remain stable across unrelated workspace mutations. File/hunk IDs copied from the current output generally remain usable across ordinary commits, so you can reference several in a row, including across chained `but commit` calls. If an ID stops resolving, re-read the diff and continue. Commit IDs are change-ID prefixes when the commit has a change ID and sha prefixes otherwise. Change-ID refs survive history edits (`amend`, `squash`, `move`, `uncommit`, `reword`); sha refs and `#N`-suffixed refs do not — a stale sha can silently resolve to the wrong commit. History edits may run in sequence off one status read when every ref involved is a change-ID ref; otherwise run them one at a time with `--status-after` to get the next ref.

**Usage:** Pass these IDs as arguments to commands:

```bash
but commit -b <branch-name> -m "message" <file-or-hunk-id>   # Commit selected changes to a branch
but amend -t <commit-id> <file-or-hunk-id> <file-or-hunk-id>  # Amend file(s) or hunk(s) into commit
but squash <commit-id> -t <commit-id> -m "message"         # Squash commits
but move <commit-id>:<file-id> --above <commit-id> -m "message" # Reposition a committed file
but move <commit-id>:<file-id>:<hunk-id> --above <commit-id> -m "message" # Reposition a committed hunk
but split <commit-id>:<file-id> -m "message"              # Extract a file immediately above its source
```

IDs are positional and space-separated. `but help cli-ids` has more detail on the basic ID kinds.

**Worktrees**: each active worktree is drawn in `but status` as a lane of its own, nested above the commit the
worktree rests on — another worktree's commit included, lanes nest recursively — or standing on
its own below the stacks when it rests outside the workspace. The lane opens with the worktree's
uncommitted area, `<id>:@ [uncommitted] {<worktree-name>}`, followed by ordinary branch rows
for the branch checked out there and the branches below it (a detached `HEAD` shows as an
anonymous segment) and their commits. The top branch's ID and the worktree's name both name the
worktree; `<worktree>:@` (that ID or the name) names its uncommitted area. `but discard` on that
top branch removes the worktree when its checkout is clean; the branches below it discard and
squash like stack branches. A worktree's branches count as applied: `but apply` on one changes
nothing, and `but unapply` refuses it — remove the worktree instead. A bare path or `dir/`
prefix names files in the checkout `but` runs from; `<worktree>:<path>` scopes a filename to that worktree
from anywhere, and `@:<path>` to the main worktree. A worktree file ID or
`<worktree>:@` works as a `but commit` change and a `but amend` or `but squash`
source: the change lands on the target and leaves that worktree's uncommitted area. `but discard`
on one discards it in that worktree, and isn't recorded in the oplog, so `but undo` can't bring
it back. Without a target flag, worktree changes commit to the tip of the worktree's own branch; an explicit target
commit or branch does not have to be the worktree's own. One operation reads from one worktree
at a time — a selection mixing worktrees is refused. A worktree's branches are also targets:
`but commit`, `but move`, and `but pick` with `-b <branch>` place the commit on that branch's tip,
and `--below <checked-out-branch-id>` on the tip of the branch the worktree has checked out
(`--above` it is refused — that is its uncommitted area). `but branch new <name> --above <worktree-branch>`
creates a branch that becomes the worktree's checked-out branch. `but move <branch> --above <branch>`
moves an existing branch between a worktree's lane and the workspace, in either direction. A branch
stacked onto the one a worktree has checked out becomes its checked-out branch; moving the checked-out
branch away leaves the worktree on the branch below it, or detached on the commit it was based on. A worktree's own commits carry ordinary commit IDs: `reword`, `move`,
`squash`, and `pick` accept them, and the worktree's branch and checkout follow the rewrite.
`but reword <branch> -m <name>` renames a worktree's branch, and a checkout on it follows the new name.
Uncommitting one lands in that worktree's uncommitted area, so `squash -t` names it by the
worktree's area ID (`<id>:@`), not `@`; `but uncommit` infers it.
IDs are the same whichever checkout `but` runs in, and `@` always means the main worktree's
area. Only what a command falls back to when no location is named depends on it: run from
inside a linked worktree, a bare `but commit` and `but diff`, and `but amend`/`but squash`
with `-t` but no sources, take that worktree's changes, while `but commit --empty` and `but pick`
without a target go on its branch, and a bare `but discard` discards that worktree's changes.
`but absorb` and `but branch new` without `--above`/`--below` refuse there instead of acting on
the main worktree, and every such default is refused in a worktree GitButler doesn't manage (it is archived, or its `HEAD`
is unusable).

## Parallel vs Stacked Branches

### Parallel Branches (Independent Work)

Create with `but branch new <name>`:

```
main ──┬── api-endpoint (independent)
       └── ui-update    (independent)
```

### Stacked Branches (Dependent Work)

**To stack an existing branch** on top of another: `but move <child-branch-name> --above <parent-branch-name>`.

**To create a new stacked branch** from scratch: `but branch new <name> --above <parent-branch-name>` — only use this when the child branch doesn't exist yet.

```
main ── authentication ── user-profile ── settings-page
        (base)            (stacked)       (stacked)
```

To tear off a branch from a stack:

```bash
but move feature/frontend --unstack
```

**Dependency tracking:** GitButler automatically tracks which changes depend on which commits. A dependent change can only be committed to the stack that contains the commits it depends on.

## The Editing Model

History editing is expressed as *sources* and a *target*. Sources are positional CLI IDs; the target
is a flag. `@` is a special ID meaning "the uncommitted area".

`but squash` carries most of the model — what it does depends on the kinds you combine:

| Sources          | Target (`-t`) | Operation                         | Example                       |
| ---------------- | ------------- | --------------------------------- | ----------------------------- |
| Commit(s)        | Commit        | Squash commits together           | `but squash mm -t nn -m "…"`  |
| Branch           | Commit        | Squash a branch into a commit     | `but squash <branch-name> -t nn -m "…"` |
| Commit(s)        | Branch        | Squash into the branch's newest   | `but squash mm -t <branch-name> -m "…"` |
| Branch           | *(none)*      | Squash the branch into one commit | `but squash <branch-name> -m "…"`       |
| Uncommitted file | Commit        | Amend the change into a commit    | `but squash a1 -t nn`         |
| `@`              | Commit        | Amend everything into a commit    | `but squash @ -t nn`                     |
| Commit           | `@`           | Uncommit the commit               | `but squash mm -t @`                     |
| Branch           | `@`           | Uncommit and remove the branch    | `but squash <branch-name> -t @`          |
| Committed change | Commit        | Move the change to another commit | `but squash nn:a -t mm`       |

**Message flags:** commits or branches compose a NEW message unless the target is `@` — always pass
`-m`. Without it an agent run keeps the composed message (the joined source messages) and a terminal
opens an editor. The remaining rows reuse the target's message and need no flag, and `-t @` rejects
message flags outright.

The two amend rows overlap with `but amend` — prefer `but amend -t nn a1`, which does only that and
takes the same IDs.

The other editing commands are narrower entry points on the same model:

- `but amend -t <commit> <changes>` — amend uncommitted changes into a known commit
- `but uncommit <commits-branches-or-committed-changes>` — move committed work back to uncommitted;
  branches are removed, and committed changes in one call must come from one commit
- `but move <sources> --above|--below|--branch|--unstack` — relocate commits, committed changes, or a
  branch; this is the command with position control. For commits or committed changes,
  `--above <branch>`, `--below <branch>`, and `--unstack` create a new branch; add `-b <new-name>`
  to name it instead of using a generated name
- `but discard <changes>` — drop work instead of relocating it

## Dependency Tracking

GitButler tracks dependencies between changes automatically.

### How It Works

```
Commit C1: Added function foo()
Commit C2: Added function bar()
Uncommitted: Edit to a line inside foo()
```

The uncommitted change **depends on** C1 because it changes lines that C1 added. Dependencies come
from overlapping lines only: new code elsewhere that calls `foo()` does not depend on C1.

**Implications:**

1. Can't commit this change to a stack that doesn't contain C1
2. When amending it into history, it belongs in C1 (or a commit after C1)
3. If you try to move the change, GitButler prevents invalid operations

### Why This Matters

Prevents you from creating broken states:

- Can't move dependent lines away from the commits they depend on
- Can't commit changes to the wrong stack
- It does not check that each branch builds on its own: a call to code on another branch is not a
  dependency

## Empty Commits as Placeholders

You can create empty commits:

```bash
but commit --empty --below nn -m "TODO: Add error handling"
but commit --empty --above nn -m "TODO: Add error handling"
```

Example workflow:

```bash
but commit --empty --below rr -m "TODO: Add error handling"
# Later, amend the error handling changes into the placeholder
but amend -t <empty-commit-id> <file-id>
```

## Operation History (Oplog)

Most GitButler mutations are recorded in the oplog (operation log). Push and `but worktree`
commands are not.

### What Gets Recorded

- Branch creation/deletion
- Commits
- Squash/amend/move/uncommit/discard operations (an amend is listed as `[SQUASH]`)
- Pull (listed as `[OTHER] Merged upstream`)

### Using Oplog

```bash
but oplog                      # View history
but undo                       # Undo last operation
but redo                       # Redo last undone operation
but oplog list --since <snapshot-id>
but oplog list --snapshot
but oplog snapshot -m "known good"
but oplog restore <snapshot-id>  # Restore to specific point
```

## Conflict Resolution Mode

When `but pull` causes conflicts, affected commits are marked as conflicted. Agents resolve them
without entering a mode, with `but resolve conflicts` and `but resolve apply` (see `but skill`).
Resolution mode below is for running code against a resolution, and it needs
`gitbutler/workspace`: while a single branch is checked out, `but resolve <commit-id>` refuses.

Enter it with `but resolve <commit-id>` (oldest conflicted commit first), edit the files, then run
`but resolve finish`, or `but resolve cancel --force` to back out and drop the edits; `but skill` has the steps. Run no other
`but` mutations until you finish or cancel.

**Conflicted submodules:** a conflicted submodule has no markers, and `but resolve finish` refuses while it is unresolved, even to keep the commit's own side. Select it explicitly: check out the wanted commit inside the submodule (or write the file that replaces it) and run `git add -- <path>`, or drop it with `git rm -- <path>`. These are the only git write commands allowed during resolution, and only for the paths `finish` names; never `git commit`. If the submodule directory is empty or missing, do not `git add` it (for a missing one that stages its removal): keep any resolution edits you want and report the conflict. `but resolve cancel --force` abandons the resolution and discards its edits; use it only when those edits can be discarded.

## Read-Only Git Commands

Git commands that don't modify state are safe to use:

**Safe (read-only):**

- `git log` - View history
- `git diff` - See changes (but prefer `but diff` — it supports CLI IDs)
- `git show` - View commits
- `git blame` - See line history
- `git reflog` - View reference log

**Don't use (in either mode):**

- `git status` - Missing the CLI IDs that agents need; with several stacks applied it shows the merged workspace state, not individual stacks
- `git commit` - On `gitbutler/workspace`, the commit lands on top of the workspace commit and `but` refuses to run until `but teardown`; use `but commit`
- `git checkout` - Breaks workspace model
- `git rebase` - Conflicts with GitButler's management
- `git merge` - `but pull` updates branches from the target; `but merge` lands a branch on the target without review
