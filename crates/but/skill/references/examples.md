# GitButler CLI Workflow Examples

Real-world examples of common workflows.

**Note on CLI IDs:** Examples use illustrative IDs such as `nn` and `a1`; copy actual IDs from current `but status` or `but diff` output. Real output may use longer IDs (e.g., `mzm:uvw:2e4`). Use full branch names for mutations. Commit change-ID prefixes survive history edits; SHA prefixes can go stale.

## Example 1: Starting Independent Parallel Work

**Scenario:** Need to work on two independent features: a new API endpoint and UI styling updates.

```bash
# 1. Make changes to multiple files
# (edit api/users.js and components/Button.svelte)

# 2. Read the file IDs of the uncommitted changes
but diff

# 3. Commit each file to its own branch; -b creates a branch that does not exist yet.
# Multiple IDs are space-separated positional arguments.
but commit -b api-endpoint -m "Add user details endpoint" <api-file-id>
but commit -b ui-styling -m "Update button hover styles" <ui-file-id>

# Follow-up fix that belongs in a commit you just made? Amend it in.
# Add --status-after only when the next step needs resulting workspace IDs or details.
# but amend -t <api-commit-id> <api-fix-file-id> <api-fix-hunk-id>

# 4. Create pull requests (auto-pushes the branches; -m sets the PR title so no editor opens)
but pr new api-endpoint -m "Add user details endpoint"
but pr new ui-styling -m "Update button hover styles"
```

**Why parallel branches?** The API endpoint and UI styling are independent - neither depends on the other. They can be reviewed and merged separately.

## Example 2: Building Stacked Features

**Scenario:** Need to add authentication, then build a user profile page that requires auth.

```bash
# 1. Update
but pull

# 2. Implement auth and commit it on a new branch
# (edit auth/login.js, auth/middleware.js)
but diff
but commit -b add-authentication -m "Add JWT authentication" <file-ids>

# 3. Create stacked branch anchored on authentication
but branch new user-profile --above add-authentication

# 4. Implement profile page (depends on auth)
# (edit pages/profile.js)
but diff
but commit -b user-profile -m "Add user profile page" <file-ids>

# 5. Create stacked pull requests from the top branch once (auto-pushes its ancestors)
but pr new user-profile -t
```

**Result:** Two PRs where user-profile targets add-authentication.

## Example 3: Amending Fixes Into Existing Commits

**Scenario:** Made a small typo fix that should be part of an existing commit, not a new commit.

```bash
# 1. Check current commits and uncommitted changes
but status -fv

# Output (abridged):
# ╭┄ @ [uncommitted]
# ┊   a1 M fix-typo.js
# ┊
# ┊╭┄ at [feature-x]
# ┊●   nn Implement feature logic
# ┊●   mm Add feature tests

# 2. Decide which commit the fix belongs to
# (the typo is in code introduced by nn, so it belongs in nn)

# 3. Amend the file into that commit
but amend -t nn a1    # Amend just this file into nn
```

**Why amend?** Keeps history clean. Small fixes belong in the commits they fix, not as separate "fix typo" commits. You know what you changed and why — pick the target commit yourself.

## Example 4: Reorganizing Commit History

### Scenario A: Squashing Commits

**Situation:** Made 5 small WIP commits, want to combine into one logical commit.

```bash
# Before (newest first):
# rr: More tweaks
# pp: Fix another thing
# nn: Fix tests
# mm: Adjust logic
# kk: Initial implementation

# Squash all commits in the branch into one
but squash feature -m "Implement feature"

# Or squash specific commits into a target commit
but squash rr pp nn mm -t kk -m "Implement feature"
```

### Scenario B: Moving Committed Changes

**Situation:** A change was committed in the wrong commit, need to move it.

```bash
# 1. See which files are in which commits
but status -fv

# Output (abridged):
# ┊●   nn Add API layer
# ┊│     nn:a1 A api.js
# ┊│     nn:a2 A utils.js
# ┊●   mm Add config
# ┊│     mm:c1 A config.js

# 2. Move utils.js from nn to mm
but squash nn:a2 -t mm -u    # Committed file nn:a2 (utils.js) → commit mm, keep mm's message

# Alternatively, move only one hunk from the file
but diff nn                  # Read the committed hunk ID
but squash nn:a2:5 -t mm     # Target message is reused

# Or put that committed hunk in a new commit at a chosen position
but move nn:a2:5 --above mm -m "Extract utility change"

# Or split it into a named commit immediately above its source
but split nn:a2:5 -m "Extract utility change"

# Or move that committed hunk back to uncommitted
but squash nn:a2:5 -t @
```

### Scenario C: Moving Commit to Different Branch

**Situation:** Committed to wrong branch, need to move commit.

```bash
# 1. Check current state
but status

# Output (abridged):
# ┊╭┄ at [feature-a]
# ┊●   nn This should be in feature-b!
# ┊●   mm Correct commit

# 2. Move the commit (-b creates feature-b if it does not exist)
but move nn -b feature-b    # Move nn to top of feature-b

# Alternatively, move onto a new named branch above feature-a
but move nn --above feature-a -b extracted-work  # --below also works

# Or move onto a new named independent branch
but move nn --unstack -b independent-work
```

## Example 5: Stacking Existing Branches

**Scenario:** Two independent branches exist, but one now depends on the other. Stack them.

```bash
# 1. Check current state — two independent branches in separate stacks
but status

# Two lanes: feature/backend (2 commits) and feature/frontend (1 commit)

# 2. Frontend now depends on backend API — stack frontend on backend
#    IMPORTANT: Use full branch names; short IDs belong to the previous snapshot
but move feature/frontend --above feature/backend

# Result: one lane, with feature/frontend stacked on feature/backend

# 3. Continue working — commits go to the right branch
but diff
but commit -b feature/backend -m "Add caching layer" <id>
but commit -b feature/frontend -m "Add dialog component" <id>
```

**Key point:** branch stack moves use full branch names like `feature/frontend`. Commit reordering still uses commit IDs.

## Example 6: Conflict Resolution

**Scenario:** After `but pull`, conflicts appear in a commit.

```bash
# 1. Pull updates
but pull

# Output (abridged):
# Summary
# ────────
#   feature-x - conflicted
#       vtxz [conflict] Add validation
#
# To resolve conflicts:
#   1. Run `but resolve conflicts` to show the conflicts of the oldest conflicted commit
#   2. Resolve each conflict with `but resolve apply`, as that output describes
# (with gitbutler/workspace checked out, the hint names `but resolve <commit>` instead;
#  the loop below works either way)

# 2. Show the conflicts of the branch's oldest conflicted commit
but resolve conflicts feature-x

# Output:
# Conflicts in commit 418aa8d on branch feature-x
#
# api/users.js
# ── conflict 1 of 1
# <<< ours (new base)
#   log();
# ||| base
#
# >>> theirs (this commit)
#   validate();

# 3. Resolve each conflict: pipe the merged content for that region,
#    or take one side with --ours / --theirs
but resolve apply api/users.js:1 <<'EOF'
  log();
  validate();
EOF

# Output:
# ✓ Resolved 1 conflict in api/users.js — 418aa8d → 6a982df
# All conflicts in this commit are resolved.
# If this isn't right, run `but undo` to revert it.

# 4. Repeat step 2 until it reports no conflicts
but resolve conflicts feature-x

# Output:
# Branch "feature-x" has no conflicted commits.
```

To build or test against a resolution, use edit mode instead (`but resolve <commit>`, edit the
files, `but resolve finish`). Edit mode needs `gitbutler/workspace` and is refused while a single
branch is checked out.

## Example 7: After a Pull Request Is Merged

**Scenario:** The PR for `user-dashboard` was merged on the forge.

```bash
but pull --check                # is user-dashboard listed as [integrated]?
but unapply user-dashboard      # only if it is not: `but pull` would rebase it instead
but pull
```

## Example 8: Working with Applied/Unapplied Branches

**Scenario:** Three branches are applied, and you want to finish one without the other two in the working tree.

```bash
# 1. Check active branches
but status

# 2. Set feature-b and feature-c aside (unapply acts on each branch's whole stack)
but unapply feature-b
but unapply feature-c

# 3. Focus on feature-a
# (make changes, then read their IDs and commit)
but diff
but commit -b feature-a -m "Complete feature-a" <file-ids>

# 4. Create PR for feature-a (auto-pushes)
but pr new feature-a -m "Complete feature-a"

# 5. Bring the other branches back; `but apply` refuses a branch that conflicts
#    with an applied stack
but apply feature-b
but apply feature-c
```

## Example 9: Fixing History Before Pushing

**Scenario:** Made several commits, realized you need to reword messages and reorder.

```bash
# 1. Current state
but status

# Commits on feature-x, newest first:
#   rr final commit
#   pp WIP
#   nn Fix stuff
#   mm Another fix
#   kk Initial

# 2. Reword commit messages — commit refs are change-ID based and stay
#    valid across rewords and other history edits
but reword pp -m "Add validation logic"
but reword nn -m "Fix edge case in parser"
but reword mm -m "Update error messages"

# 3. Move rr to be earlier
but move rr --below nn    # Place rr directly below nn

# 4. Squash similar commits
but squash mm -t nn -u    # Combine error handling commits; -u keeps nn's message, drops mm's

# Commits on feature-x, newest first:
#   pp Add validation logic
#   nn Fix edge case in parser
#   rr final commit
#   kk Initial

# 5. Push clean history
but push feature-x
```

## Example 10: Recovering from Mistakes

**Scenario:** Made changes you didn't mean to, need to undo.

### Undo Last Operation

```bash
# Made a mistake
but squash feature -m "..."    # Oops! Didn't mean to squash

# Undo it
but undo         # Reverts the squash
```

### Restore to Earlier Point

```bash
# View operation history
but oplog

# Output (snapshot refs are git SHAs, not CLI IDs; an amend is listed as [SQUASH]):
# Operations History
# ──────────────────────────────────────────────────
# 9c1f2ab 2026-01-02 10:15:03 [SQUASH] Squashed commit
# f8a3733 2026-01-02 10:14:41 [COMMIT] Created commit
# 1d5c806 2026-01-02 10:13:30 [BRANCH] Created branch

# Each entry restores the state from before its own operation, so restore
# the squash entry to get back to just before the squash
but oplog restore 9c1f2ab
```

### Discard Uncommitted Changes

```bash
# Changed a file but want to discard
but status -fv

# Output (abridged):
# ╭┄ @ [uncommitted]
# ┊   a1 M bad-changes.js

# Discard it
but discard a1
```

## Tips and Tricks

### Preview Before Doing

```bash
but push my-feature --dry-run   # See what would be pushed
```

### Multiple Commits From One Diff

Chain `but commit` calls to split a dirty diff into several commits off one `but diff` read:

```bash
but diff   # read the file/hunk IDs once

but commit -b my-branch -m "Add parser" qs:5 qs:2 \
  && but commit -b my-branch -m "Add tests" uo:d
```

The commits stack in the order you write them, so `Add parser` ends up below (older
than) `Add tests`.

### Viewing History

```bash
but show my-feature      # Show all commits in branch
git log my-feature       # Traditional git log (read-only; takes branch names, not CLI IDs)
```
