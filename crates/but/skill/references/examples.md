# GitButler CLI Workflow Examples

Real-world examples of common workflows.

**Note on CLI IDs:** Examples below use full branch names for branch-targeting mutations. Illustrative IDs like `nn` and `a1` keep other commands readable; in practice, **always read actual IDs from `but status -fv`** because they are generated for the current workspace snapshot. Commit IDs are short change-ID prefixes that stay stable across history edits (e.g., `kyn`; commits without a change ID fall back to a sha prefix), and file/hunk/stack IDs are auto-generated (e.g., `r`, `r:c`, `h0`). All IDs are unique across entity types within one snapshot.

## Example 1: Starting Independent Parallel Work

**Scenario:** Need to work on two independent features: a new API endpoint and UI styling updates.

```bash
# 1. Check current state
but status -fv

# 2. Create two independent (parallel) branches
but branch new api-endpoint
but branch new ui-styling

# 3. Make changes to multiple files
# (edit api/users.js and components/Button.svelte)

# 4. Check what's uncommitted
but status -fv

# 5. Commit specific files by passing their CLI IDs (recommended for agents)
# Use full branch names plus file IDs from but status -fv output
# Multiple IDs are space-separated positional arguments.
but commit -b api-endpoint -m "Add user details endpoint" <api-file-id>
but commit -b ui-styling -m "Update button hover styles" <ui-file-id>

# Follow-up fix that belongs in a commit you just made? Amend it in.
# Add --status-after only when the next step needs resulting workspace IDs or details.
# but amend -t <api-commit-id> <api-fix-file-id> <api-fix-hunk-id>

# 6. Create pull requests (auto-pushes the branches; -m sets the PR title so no editor opens)
but pr new api-endpoint -m "Add user details endpoint"
but pr new ui-styling -m "Update button hover styles"
```

**Why parallel branches?** The API endpoint and UI styling are independent - neither depends on the other. They can be reviewed and merged separately.

## Example 2: Building Stacked Features

**Scenario:** Need to add authentication, then build a user profile page that requires auth.

```bash
# 1. Check current state and update
but pull
but status -fv

# 2. Create base branch for authentication
but branch new add-authentication

# 3. Implement auth and commit
# (edit auth/login.js, auth/middleware.js)
but status -fv
but commit -b add-authentication -m "Add JWT authentication" <file-ids>

# 4. Create stacked branch anchored on authentication
but branch new user-profile -a add-authentication

# 5. Implement profile page (depends on auth)
# (edit pages/profile.js)
but status -fv
but commit -b user-profile -m "Add user profile page" <file-ids>

# 6. Create stacked pull requests from the top branch once (auto-pushes its ancestors)
but pr new user-profile -t
```

**Result:** Two PRs where user-profile targets add-authentication, with GitButler stack information in the PR descriptions.

## Example 3: Amending Fixes Into Existing Commits

**Scenario:** Made a small typo fix that should be part of an existing commit, not a new commit.

```bash
# 1. Check current commits and uncommitted changes
but status -fv

# Output shows:
# Branch: feature-x (bu)
# Commits:
#   nn: Implement feature logic
#   mm: Add feature tests
# Uncommitted:
#   a1: fix-typo.js

# 2. Decide which commit the fix belongs to
# (the typo is in code introduced by nn, so it belongs in nn)

# 3. Amend the file into that commit
but amend -t nn a1    # Amend just this file + get updated status
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

### Scenario B: Moving Files Between Commits

**Situation:** A file was committed in the wrong commit, need to move it.

```bash
# 1. See which files are in which commits
but status -fv

# Output shows:
# nn: Add API layer
#   nn:a1  api.js
#   nn:a2  utils.js
# mm: Add config
#   mm:c1  config.js

# 2. Move utils.js from nn to mm
but squash nn:a2 -t mm -u    # Committed file nn:a2 (utils.js) → commit mm, keep mm's message
```

### Scenario C: Moving Commit to Different Branch

**Situation:** Committed to wrong branch, need to move commit.

```bash
# 1. Check current state
but status -fv

# Output:
# Branch: feature-a (bu)
#   nn: This should be in feature-b!
#   mm: Correct commit

# 2. Create or identify target branch
but branch new feature-b    # Creates branch bv

# 3. Move the commit
but move nn -b feature-b    # Move nn to top of feature-b
```

## Example 5: Stacking Existing Branches

**Scenario:** Two independent branches exist, but one now depends on the other. Stack them.

```bash
# 1. Check current state — two independent branches in separate stacks
but status -fv

# Output:
# Stack 1: feature/backend (bu) — 2 commits
# Stack 2: feature/frontend (bv) — 1 commit

# 2. Frontend now depends on backend API — stack frontend on backend
#    IMPORTANT: Use full branch names; short IDs belong to the previous snapshot
but move feature/frontend --above feature/backend

# Result: Both branches are now in the same stack:
# Stack 1: feature/backend → feature/frontend (stacked)

# 3. Continue working — commits go to the right branch
but status -fv
but commit -b feature/backend -m "Add caching layer" <id>
but commit -b feature/frontend -m "Add dialog component" <id>
```

**Key point:** branch stack moves use full branch names like `feature/frontend`. Commit reordering still uses commit IDs.

## Example 6: Conflict Resolution

**Scenario:** After `but pull`, conflicts appear in a commit.

```bash
# 1. Pull updates
but pull

# Output:
# Summary
# ────────
#   feature-x - conflicted
#       nn Add validation

# 2. Enter resolution mode using the commit ID from the pull output
but resolve nn

# Output:
# Checking out conflicted commit nn
# Conflicted files remaining:
#   ✗ api/users.js
#      12│<<<<<<< New base: ...
#      ...conflict regions with line numbers...

# 3. Edit each conflicted file to resolve
# IMPORTANT: You MUST edit the files — do NOT just run `but resolve finish`
# NEVER use `git add`, `git checkout --theirs/--ours`, or any git write command — just edit the files directly with the Edit tool, then `but resolve finish`
# (edit to remove every marker — <<<<<<< ||||||| ======= >>>>>>> — and keep correct content;
#  with several conflicted files, `but resolve status` re-lists what remains)

# 4. Finalize
but resolve finish

# Output:
# ✓ Conflict resolution finalized successfully!
# No conflict markers remain in the resolved files.
# Workspace restored; uncommitted changes intact: ...
# No follow-up status or marker scan needed — finish already reports both.
```

## Example 7: Complete Feature Development Workflow

**Scenario:** Building a complete feature from start to finish.

```bash
# 1. Update to latest
but pull

# 2. Create branch for feature
but branch new user-dashboard

# 3. Make initial changes
# (create dashboard.js, add routes)

# 4. Check status and gather file IDs
but status -fv

# 5. First commit
but commit -b user-dashboard -m "Add dashboard route and basic layout" <file-ids>

# 6. Continue iterating
# (add widgets, styling)
but commit -b user-dashboard -m "Add dashboard widgets" <file-ids>
but commit -b user-dashboard -m "Style dashboard components" <file-ids>

# 7. Make small fix
# (fix typo in widget)
but amend -t <commit-id> a1    # Amend fix into the commit it belongs to

# 8. Clean up if needed
but squash user-dashboard -m "Add user dashboard"    # Combine all commits (optional)

# 9. Create pull request (auto-pushes the branch)
but pr new user-dashboard -m "Add user dashboard"

# Output:
# Created PR #123: https://github.com/org/repo/pull/123

# 10. After PR is merged, update
but pull
```

## Example 8: Working with Applied/Unapplied Branches

**Scenario:** Have 3 branches, but two are causing conflicts. Temporarily unapply them.

```bash
# 1. Check active branches
but status -fv

# Output:
# Applied branches:
#   bu: feature-a
#   bv: feature-b
#   bw: feature-c

# 2. Conflicts between feature-b and feature-c
# Unapply them temporarily
but unapply feature-b
but unapply feature-c

# 3. Focus on feature-a
# (make changes, commit)
but commit -b feature-a -m "Complete feature-a" <file-ids>

# 4. Create PR for feature-a (auto-pushes)
but pr new feature-a -m "Complete feature-a"

# 5. Reapply other branches
but apply feature-b
but apply feature-c

# 6. Deal with their conflicts now
but resolve ...
```

## Example 9: Fixing History Before Pushing

**Scenario:** Made several commits, realized you need to reword messages and reorder.

```bash
# 1. Current state
but status -fv

# Output (newest first):
# Branch: feature-x (bu)
#   rr: final commit
#   pp: WIP
#   nn: Fix stuff
#   mm: Another fix
#   kk: Initial

# 2. Reword commit messages — commit refs are change-ID based and stay
#    valid across rewords and other history edits
but reword pp -m "Add validation logic"
but reword nn -m "Fix edge case in parser"
but reword mm -m "Update error messages"

# 3. Move rr to be earlier
but move rr --below nn    # Place rr directly below nn

# 4. Squash similar commits
but squash mm -t nn -u    # Combine error handling commits; -u keeps nn's message, drops mm's

# Output (newest first):
# Branch: feature-x (bu)
#   pp: Add validation logic
#   nn: Fix edge case in parser
#   rr: final commit
#   kk: Initial

# 5. Push clean history
but push feature-x
```

## Example 10: Daily Development Workflow

**Typical day working with GitButler:**

```bash
# Morning: Start day
but pull    # Get latest from team

# Start new task
but branch new fix-auth-bug  # Create branch for today's work

# Work and commit iteratively
# (make changes)
but status -fv              # Check changes
but commit -b fix-auth-bug -m "Identify auth bug source" <file-ids>
# (make more changes)
but commit -b fix-auth-bug -m "Fix token expiration handling" <file-ids>
# (small fix to existing code)
but amend -t <commit-id> a1  # Amend fix into the commit it belongs to

# Mid-day: Start urgent fix on different branch
but branch new hotfix-login  # Parallel branch for urgent work
# (make fix)
but commit -b hotfix-login -m "Fix login redirect loop" <file-ids>
but pr new hotfix-login -m "Fix login redirect loop"    # Push and create PR immediately

# Back to original work
# (continue working on fix-auth-bug)
but commit -b fix-auth-bug -m "Add tests for token handling" <file-ids>

# End of day: Clean up and create PR
but squash fix-auth-bug -m "Fix auth bug"    # Combine into clean history
but pr new fix-auth-bug -m "Fix auth bug"    # Push and create PR

# After PR review: Make requested changes
# (make changes based on feedback)
but amend -t <commit-id> <file-id>  # Amend each fix into the commit it belongs to
but push fix-auth-bug   # Push updated history
```

## Example 11: Recovering from Mistakes

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

# Output (snapshot refs are git SHAs, not CLI IDs):
# 9c1f2ab 2026-01-02 [SQUASH] Squashed commits
# f8a3733 2026-01-02 [COMMIT] Created commit
# 4b70e19 2026-01-02 [AMEND] Amended commit
# 1d5c806 2026-01-02 [BRANCH] Created branch

# Restore to before the squash, using the SHA from the output
but oplog restore f8a3733
```

### Discard Uncommitted Changes

```bash
# Changed a file but want to discard
but status -fv

# Output:
# Uncommitted:
#   a1: bad-changes.js

# Discard it
but discard a1
```

## Tips and Tricks

### Quick Status Check

```bash
but status -fv    # File-centric view for quick overview
```

### Preview Before Doing

```bash
but push my-feature --dry-run   # See what would be pushed
```

### Multiple Commits From One Diff

File/hunk IDs copied from the original output generally remain usable across
commits. Chain `but commit` calls to split a dirty diff into several commits in
one go:

```bash
but diff   # read the file/hunk IDs once

but commit -b my-branch -m "Add parser" qs:5 qs:2 \
  && but commit -b my-branch -m "Add tests" uo:d
```

The commits stack in the order you write them, so `Add parser` ends up below (older
than) `Add tests`. Chain these commit commands when each references uncommitted IDs
and a full branch name. If an ID stops resolving, re-read the diff and continue.
Mutation output is concise by default. Add `--status-after` only when the next
step needs workspace IDs or details that the mutation result does not provide.
History edits — `amend`, `squash`, `move`, `uncommit`, `reword` — may also run in
sequence off one status read when every commit ref involved is a change-ID ref;
those stay stable across the edits. Run them one at a time when a ref is sha-based
or `#N`-suffixed, or when the next command needs freshly issued IDs, and add
`--status-after` to get them.


### Auto-completion

```bash
eval "$(but completions zsh)"     # Add to ~/.zshrc
eval "$(but completions bash)"    # Add to ~/.bashrc
```

### Viewing History

```bash
but show bu       # Show all commits in branch
git log bu               # Traditional git log (read-only, still works)
```
