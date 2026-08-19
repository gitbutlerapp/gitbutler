# GitButler CLI Key Concepts

Deep dive into GitButler's conceptual model and philosophy.

## The Workspace Model

### Traditional Git: Serial Branching

```
main ──┬── feature-a (checkout here, work, commit, checkout back)
       └── feature-b (checkout here, work, commit, checkout back)
```

- Work on ONE branch at a time
- Switch contexts with `git checkout`
- Changes are isolated by branch

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

### Key Implications

1. **No `git checkout`**: You don't switch between branches. All applied branches exist simultaneously in your workspace.

2. **The `gitbutler/workspace` branch**: A merge commit containing all applied stacks. Don't interact with it directly - use `but` commands.

3. **Applied vs Unapplied**: Control which branches are active:
   - Applied branches: In your working directory
   - Unapplied branches: Exist but not active
   - Use `but apply`/`but unapply` to control

## CLI IDs: Short Identifiers

Every object gets a short, human-readable CLI ID shown in `but status`. IDs are generated per-session and are unique across all entity types (no two objects share an ID) — always read them from `but status`.

```
Commits:    1, kyn, mpq#0  (short change-ID prefix when the commit has one, sha prefix otherwise;
                             a #N suffix disambiguates commits sharing a change ID)
Branches:   fe, bu, ui     (unique 2–3 char substring of the branch name, e.g. "fe" from "feature-x";
                             falls back to auto-generated ID if no unique substring exists)
Files:      g, qs, uo      (derived from the file path, long enough to be unique)
Hunks:      g:5, uo:d      (<file-id>:<hunk-id>; the hunk part is derived from the hunk's content)
Committed files: kyn:n     (<commit-id>:<file-id>, shown under each commit in `but status -fv`)
Stacks:     m0, n0          (auto-generated, 2–3 chars)
```

**Why?** Git commit SHAs are long (40 chars). CLI IDs are short, variable-length, and unique within your current workspace context. Commits, files, and hunks may use a single character when that is unambiguous.

**Reading status output:** the first token on each line is that line's ID. Verbose commit lines append an informational `(sha …)` after the timestamp — it changes on every amend; do not pass it to commands.

**Stability:** Branch short IDs identify branches only within the workspace snapshot that produced them; branch names remain stable across unrelated workspace mutations. File/hunk IDs copied from the current output generally remain usable across ordinary commits, so you can reference several in a row, including across chained `but commit` calls. If an ID stops resolving, re-read the diff and continue. Commit IDs are change-ID prefixes when the commit has a change ID and sha prefixes otherwise. Change-ID refs survive history edits (`amend`, `squash`, `move`, `uncommit`, `reword`); sha refs and `#N`-suffixed refs do not — a stale sha can silently resolve to the wrong commit. History edits may run in sequence off one status read when every ref involved is a change-ID ref; otherwise run them one at a time with `--status-after` to get the next ref.

**Usage:** Pass these IDs as arguments to commands:

```bash
but commit -b <branch-name> -m "message" <file-or-hunk-id>   # Commit selected changes to a branch
but amend -t <commit-id> <file-or-hunk-id> <file-or-hunk-id>  # Amend file(s) or hunk(s) into commit
but squash <commit-id> -t <commit-id> -m "message"         # Squash commits
```

IDs are positional and space-separated. `but help cli-ids` documents every ID kind in detail.

**Linked worktrees** (experimental, only with the `worktreeManipulation` feature flag on): each
active linked worktree gets its own ID and is drawn in `but status` as a lane — a braced
`{<branch>}` heading (the worktree name when its `HEAD` is detached) nested above the commit the
worktree rests on — another worktree's commit included, lanes nest recursively — or standing on
its own below the stacks when it rests outside the workspace.
The lane lists that checkout's uncommitted files and the commits the worktree owns; in `--json`
they appear in a top-level `worktrees` array. The worktree ID on the heading names its whole
uncommitted area the way `zz` names the main worktree's, and `<worktree-name>:<path>` scopes a
filename to that checkout — `zz:<path>` keeps meaning the main worktree. A filename dirty in
several checkouts at once is ambiguous; the error suggests the scoped forms. A worktree file or
heading ID — like `zz` for the main checkout — works as a `but commit` change and a `but amend`
source: the change lands on the target and leaves that worktree's uncommitted area. Without a
target flag, worktree changes commit to the tip of the worktree's own branch; an explicit target
commit or branch does not have to be the worktree's own. One operation reads from one checkout
at a time — a selection mixing checkouts is refused. A worktree is also a target: `but commit`,
`but move`, and `but pick` with `-b <worktree-id-or-its-branch-name>` or `--below <worktree-id>`
place the commit on the tip of the branch the worktree has checked out (`--above` is refused —
that is its uncommitted area). A worktree's own commits carry ordinary commit IDs: `reword`, `move`,
`squash`, and `pick` accept them, and the worktree's branch and checkout follow the rewrite.

## Parallel vs Stacked Branches

### Parallel Branches (Independent Work)

Create with `but branch new <name>`:

```
main ──┬── api-endpoint (independent)
       └── ui-update    (independent)
```

Use when:

- Tasks don't depend on each other
- Can be merged independently
- No shared code between them

Example: Adding a new API endpoint and updating button styles are independent.

### Stacked Branches (Dependent Work)

**To stack an existing branch** on top of another: `but move <child-branch-name> --above <parent-branch-name>`.

**To create a new stacked branch** from scratch: `but branch new <name> -a <anchor>` — only use this when the child branch doesn't exist yet.

```
main ── authentication ── user-profile ── settings-page
        (base)            (stacked)       (stacked)
```

Use when:

- Feature B needs code from Feature A
- Building incrementally on previous work
- Creating a series of related changes

Example: User profile page needs authentication to be implemented first.

**Stacking two existing branches:** If both branches already exist and you need to make one depend on the other, use top-level `move`:
```bash
but move feature/frontend --above feature/backend
# Now frontend is stacked on top of backend — both in the same stack
```

To tear off a branch from a stack:

```bash
but move feature/frontend --unstack
```

**Dependency tracking:** GitButler automatically tracks which changes depend on which commits. A dependent change can only be committed to the stack that contains the commits it depends on.

## The Editing Model

History editing is expressed as *sources* and a *target*. Sources are positional CLI IDs; the target
is a flag. `zz` is a special ID meaning "the uncommitted area".

`but squash` carries most of the model — what it does depends on the kinds you combine:

| Sources          | Target (`-t`) | Operation                         | Example                       |
| ---------------- | ------------- | --------------------------------- | ----------------------------- |
| Commit(s)        | Commit        | Squash commits together           | `but squash mm -t nn -m "…"`  |
| Branch           | Commit        | Squash a branch into a commit     | `but squash <branch-name> -t nn -m "…"` |
| Commit(s)        | Branch        | Squash into the branch's newest   | `but squash mm -t <branch-name> -m "…"` |
| Branch           | *(none)*      | Squash the branch into one commit | `but squash <branch-name> -m "…"`       |
| Uncommitted file | Commit        | Amend the change into a commit    | `but squash a1 -t nn`         |
| `zz`             | Commit        | Amend everything into a commit    | `but squash zz -t nn`         |
| Commit           | `zz`          | Uncommit the commit               | `but squash mm -t zz`         |
| Branch           | `zz`          | Uncommit and remove the branch    | `but squash <branch-name> -t zz`         |
| Committed file   | Commit        | Move the file to another commit   | `but squash nn:a -t mm`       |

**Message flags:** commits or branches compose a NEW message unless the target is `zz`, so without
`-m` they open an editor and block — always pass one. The remaining rows reuse the target's message
and need no flag, and `-t zz` rejects message flags outright.

The two amend rows overlap with `but amend` — prefer `but amend -t nn a1`, which does only that and
takes the same IDs. Reach for `squash` when the sources are commits, branches, or committed files,
which `amend` does not accept.

The other editing commands are narrower entry points on the same model:

- `but amend -t <commit> <changes>` — amend uncommitted files/hunks into a known commit
- `but uncommit <commits-branches-or-committed-files>` — move committed work back to uncommitted;
  branches are removed, and committed files in one call must come from one commit
- `but move <sources> --above|--below|--branch|--unstack` — relocate commits, committed files, or a
  branch; this is the command with position control
- `but discard <changes>` — drop work instead of relocating it

## Dependency Tracking

GitButler tracks dependencies between changes automatically.

### How It Works

```
Commit C1: Added function foo()
Commit C2: Added function bar()
Uncommitted: Call to foo() in new code
```

The uncommitted change **depends on** C1 (because it calls `foo()`).

**Implications:**

1. Can't commit this change to a stack that doesn't contain C1
2. When amending it into history, it belongs in C1 (or a commit after C1)
3. If you try to move the change, GitButler prevents invalid operations

### Why This Matters

Prevents you from creating broken states:

- Can't move dependent code away from its dependencies
- Can't commit changes to the wrong stack
- Ensures each branch remains independently functional

## Empty Commits as Placeholders

You can create empty commits:

```bash
but commit --empty --below nn -m "TODO: Add error handling"
but commit --empty --above nn -m "TODO: Add error handling"
```

**Use cases:**

1. **Mark future work:** Create empty commit as placeholder for changes you'll make
2. **Organize history:** Add semantic markers in commit history

Example workflow:

```bash
but commit --empty --below rr -m "TODO: Add error handling"
# Later, amend the error handling changes into the placeholder
but amend -t <empty-commit-id> <file-id>
```

## Operation History (Oplog)

Every operation in GitButler is recorded in the oplog (operation log).

### What Gets Recorded

- Branch creation/deletion
- Commits
- Squash/amend/move/uncommit/discard operations
- Push/pull operations

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

Think of it as "git reflog" but for all GitButler operations, not just branch movements.

**Safety net:** Made a mistake? `but undo` it. Experimented and want to go back? `but oplog restore` to earlier snapshot.

## Applied vs Unapplied Branches

Branches can be in two states:

### Applied Branches

- Active in your workspace
- Merged into `gitbutler/workspace`
- Changes visible in working directory
- Can make changes and commit

### Unapplied Branches

- Exist but not active
- Not in working directory
- Can't make changes (must apply first)
- Useful for temporarily setting aside work

### Controlling State

```bash
but apply <branch-name>    # Make branch active
but unapply <branch-name>  # Make branch inactive
```

**Use cases:**

- Unapply branches causing conflicts
- Focus on subset of work (unapply others)
- Temporarily set aside work without deleting

## Conflict Resolution Mode

When `but pull` causes conflicts, affected commits are marked as conflicted.

### Resolution Workflow

1. **Identify:** the `but pull` summary lists each conflicted commit's ID, oldest first (`but status` also shows them)
2. **Enter mode:** `but resolve <commit-id>` — it prints the conflict regions with line numbers. With several conflicted commits, resolve the oldest first: finishing a lower commit rebases the ones above it
3. **Fix conflicts:** Edit files, remove conflict markers (`but resolve status` re-lists what remains when several files are conflicted)
4. **Finalize:** `but resolve finish` or `but resolve cancel` — finish reports leftover markers and the surviving uncommitted changes, so no follow-up check is needed

### During Resolution

- You're in a special mode focused on that commit
- Other GitButler operations are limited
- `but status` shows you're in resolution mode
- Must finish or cancel before continuing normal work

## Read-Only Git Commands

Git commands that don't modify state are safe to use:

**Safe (read-only):**

- `git log` - View history
- `git diff` - See changes (but prefer `but diff` — it supports CLI IDs)
- `git show` - View commits
- `git blame` - See line history
- `git reflog` - View reference log

**Don't use in a GitButler workspace:**

- `git status` - Misleading: shows merged workspace state, not individual stacks; missing CLI IDs that agents need
- `git commit` - Commits to the workspace merge commit, not your branch
- `git checkout` - Breaks workspace model
- `git rebase` - Conflicts with GitButler's management
- `git merge` - Use `but land` instead

**Rule of thumb:** If it reads, it's fine. If it writes, use `but` instead.
