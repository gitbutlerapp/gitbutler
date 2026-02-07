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

2. **Multiple staging areas**: Each branch is like having its own `git add` staging area. You stage files to specific branches.

3. **The `gitbutler/workspace` branch**: A merge commit containing all applied stacks. Don't interact with it directly - use `but` commands.

4. **Applied vs Unapplied**: Control which branches are active:
   - Applied branches: In your working directory
   - Unapplied branches: Exist but not active
   - Use `but apply`/`but unapply` to control

## CLI IDs: Short Identifiers

Every object gets a short, human-readable CLI ID shown in `but status`. IDs are generated per-session and are unique across all entity types (no two objects share an ID) — always read them from `but status --json`.

```
Commits:    1b, 8f, c2     (short hex prefixes of the SHA, long enough to be unique)
Branches:   fe, bu, ui     (unique 2–3 char substring of the branch name, e.g. "fe" from "feature-x";
                             falls back to auto-generated ID if no unique substring exists)
Files:      g0, h0, i0     (auto-generated, 2–3 chars)
Hunks:      j0, k1, l2     (auto-generated, 2–3 chars)
Stacks:     m0, n0          (auto-generated, 2–3 chars)
```

**Why?** Git commit SHAs are long (40 chars). CLI IDs are short (2-3 chars) and unique within your current workspace context.

**Usage:** Pass these IDs as arguments to commands:

```bash
but commit <branch-id> -m "message"      # Commit to branch
but stage <file-id> <branch-id>          # Stage file to branch
but rub <commit-id> <commit-id>          # Squash commits
```

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

Create with `but branch new <name> -a <anchor>`:

```
main ── authentication ── user-profile ── settings-page
        (base)            (stacked)       (stacked)
```

Use when:

- Feature B needs code from Feature A
- Building incrementally on previous work
- Creating a series of related changes

Example: User profile page needs authentication to be implemented first.

**Dependency tracking:** GitButler automatically tracks which changes depend on which commits. You can't stage dependent changes to the wrong branch.

## Multiple Staging Areas

Traditional git has ONE staging area:

```bash
git add file1.js    # Stage to THE staging area
git add file2.js    # Stage to THE staging area
git commit          # Commit from THE staging area
```

GitButler has MULTIPLE staging areas (one per branch):

```bash
but stage file1.js api-branch    # Stage to api-branch's staging area
but stage file2.js ui-branch     # Stage to ui-branch's staging area
but commit api-branch -m "..."   # Commit from api-branch's staging area
but commit ui-branch -m "..."    # Commit from ui-branch's staging area
```

**Unstaged changes:** Files not staged to any branch yet. Use `but status` to see them, then `but stage` to assign them.

**Auto-assignment:** If only one branch is applied, changes may auto-assign to it.

## The `but rub` Philosophy

`but rub` is the core primitive operation: "rub two things together" to perform an action.

### What Happens Based on Types

The operation performed depends on what you combine:

```
SOURCE ↓ / TARGET →  │ zz (unassigned) │ Commit     │ Branch      │ Stack
─────────────────────┼─────────────────┼────────────┼─────────────┼────────────
File/Hunk            │ Unstage         │ Amend      │ Stage       │ Stage
Commit               │ Undo            │ Squash     │ Move        │ -
Branch (all changes) │ Unstage all     │ Amend all  │ Reassign    │ Reassign
Stack (all changes)  │ Unstage all     │ -          │ Reassign    │ Reassign
Unassigned (zz)      │ -               │ Amend all  │ Stage all   │ Stage all
File-in-Commit       │ Uncommit        │ Move       │ Uncommit & assign │ -
```

`zz` is a special target meaning "unassigned" (no branch).

**Common examples:**

| Source | Target | Operation | Example |
|--------|--------|-----------|---------|
| File | Branch | Stage file to branch | `but rub a1 bu` |
| File | Commit | Amend file into commit | `but rub a1 c3` |
| Commit | Commit | Squash commits | `but rub c2 c3` |
| Commit | Branch | Move commit to branch | `but rub c2 bu` |
| File | `zz` | Unstage file | `but rub a1 zz` |
| Commit | `zz` | Undo commit | `but rub c2 zz` |
| `zz` | Branch | Stage all unassigned | `but rub zz bu` |

### Higher-Level Conveniences

These commands are wrappers around `but rub`:

- `but stage <file> <branch>` = `but rub <file> <branch>`
- `but amend <file> <commit>` = `but rub <file> <commit>`
- `but squash` = Multiple `but rub <commit> <commit>` operations
- `but move` = `but rub <commit> <target>` with position control

**Why this design?** One powerful primitive is easier to understand and maintain than many specialized commands. Once you understand `but rub`, you understand the editing model.

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

1. Can't stage this change to a branch that doesn't have C1
2. `but absorb` will automatically amend it into C1 (or a commit after C1)
3. If you try to move the change, GitButler prevents invalid operations

### Why This Matters

Prevents you from creating broken states:

- Can't move dependent code away from its dependencies
- Can't stage changes to wrong branches
- Ensures each branch remains independently functional

## Empty Commits as Placeholders

You can create empty commits:

```bash
but commit empty --before c3
but commit empty --after c3
```

**Use cases:**

1. **Mark future work:** Create empty commit as placeholder for changes you'll make
2. **Mark targets:** Use with `but mark <empty-commit-id>` so future changes auto-amend into it
3. **Organize history:** Add semantic markers in commit history

Example workflow:

```bash
but commit empty -m "TODO: Add error handling" --before c5
but mark <empty-commit-id>
# Now work on error handling, changes auto-amend into the placeholder
```

## Auto-Staging and Auto-Commit (Marks)

Set a "mark" on a branch or commit to automatically organize new changes.

### Mark a Branch

```bash
but mark <branch-id>
```

New unstaged changes automatically stage to this branch. Useful when focused on one feature.

### Mark a Commit

```bash
but mark <commit-id>
```

New changes automatically amend into this commit. Useful for iterative refinement.

### Remove Marks

```bash
but mark <id> --delete    # Remove specific mark
but unmark                # Remove all marks
```

**Example workflow:**

```bash
but branch new refactor
but mark <refactor-branch-id>
# Make lots of changes - they all auto-stage to refactor branch
but unmark
```

## Operation History (Oplog)

Every operation in GitButler is recorded in the oplog (operation log).

### What Gets Recorded

- Branch creation/deletion
- Commits
- Stage operations
- Rub/squash/move operations
- Push/pull operations

### Using Oplog

```bash
but oplog                      # View history
but undo                       # Undo last operation
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
- Can make changes, commit, stage files

### Unapplied Branches

- Exist but not active
- Not in working directory
- Can't make changes (must apply first)
- Useful for temporarily setting aside work

### Controlling State

```bash
but apply <id>             # Make branch active
but unapply <id>           # Make branch inactive
```

**Use cases:**

- Unapply branches causing conflicts
- Focus on subset of work (unapply others)
- Temporarily set aside work without deleting

## Conflict Resolution Mode

When `but pull` causes conflicts, affected commits are marked as conflicted.

### Resolution Workflow

1. **Identify:** `but status` shows conflicted commits
2. **Enter mode:** `but resolve <commit-id>`
3. **Fix conflicts:** Edit files, remove conflict markers
4. **Check:** `but resolve status` shows remaining conflicts
5. **Finalize:** `but resolve finish` or `but resolve cancel`

### During Resolution

- You're in a special mode focused on that commit
- Other GitButler operations are limited
- `but status` shows you're in resolution mode
- Must finish or cancel before continuing normal work

## Read-Only Git Commands

Git commands that don't modify state are safe to use:

**Safe (read-only):**

- `git log` - View history
- `git diff` - See changes (but prefer `but diff` — it supports CLI IDs and `--json`)
- `git show` - View commits
- `git blame` - See line history
- `git reflog` - View reference log

**Don't use in a GitButler workspace:**

- `git status` - Misleading: shows merged workspace state, not individual stacks; missing CLI IDs that agents need
- `git commit` - Commits to wrong place (bypasses branch assignment)
- `git checkout` - Breaks workspace model
- `git rebase` - Conflicts with GitButler's management
- `git merge` - Use `but merge` instead

**Rule of thumb:** If it reads, it's fine. If it writes, use `but` instead.
