# GitButler CLI Workflow Examples

Real-world examples of common workflows.

## Example 1: Starting Independent Parallel Work

**Scenario:** Need to work on two independent features: a new API endpoint and UI styling updates.

```bash
# 1. Check current state
but status --json

# 2. Create two independent (parallel) branches
but branch new api-endpoint
but branch new ui-styling

# 3. Make changes to multiple files
# (edit api/users.js and components/Button.svelte)

# 4. Check what's unstaged
but status --json

# 5. Stage files to appropriate branches
but stage a1 bu    # api/users.js → api-endpoint branch
but stage a2 bv    # Button.svelte → ui-styling branch

# 6. Commit each branch (--only to commit only staged files)
but commit bu --only -m "Add user details endpoint"
but commit bv --only -m "Update button hover styles"

# 7. Push branches independently (optional, can skip if using pr new)
but push bu
but push bv

# 8. Create pull requests (auto-pushes if not already pushed)
but pr new bu
but pr new bv
```

**Why parallel branches?** The API endpoint and UI styling are independent - neither depends on the other. They can be reviewed and merged separately.

## Example 2: Building Stacked Features

**Scenario:** Need to add authentication, then build a user profile page that requires auth.

```bash
# 1. Check current state and update
but pull
but status --json

# 2. Create base branch for authentication
but branch new add-authentication

# 3. Implement auth and commit
# (edit auth/login.js, auth/middleware.js)
but status --json
but stage <file-ids> bu  # Stage changes to auth branch
but commit bu --only -m "Add JWT authentication"

# 4. Create stacked branch anchored on authentication
but branch new user-profile -a bu

# 5. Implement profile page (depends on auth)
# (edit pages/profile.js)
but status --json
but stage <file-ids> bv  # Stage changes to profile branch
but commit bv --only -m "Add user profile page"

# 6. Push both branches (maintains stack relationship)
but push
```

**Result:** Two PRs where user-profile PR depends on authentication PR. GitHub/GitLab shows the dependency.

## Example 3: Using Absorb Instead of New Commits

**Scenario:** Made a small typo fix that should be part of the last commit, not a new commit.

```bash
# 1. Check current commits and unstaged changes
but status --json

# Output shows:
# Branch: feature-x (bu)
# Commits:
#   c3: Implement feature logic
#   c2: Add feature tests
# Unstaged:
#   a1: fix-typo.js (staged to bu)

# 2. Preview what absorb would do (recommended first step)
but absorb a1 --dry-run    # Shows where a1 would be absorbed

# 3. Absorb the specific file into appropriate commit
but absorb a1              # Absorb just this file

# GitButler analyzes the change and amends it into c3
# (because the typo is in code from c3)
```

**Targeted vs blanket absorb:**

```bash
but absorb a1              # Absorb specific file (recommended)
but absorb bu              # Absorb all changes staged to branch bu
but absorb                 # Absorb ALL uncommitted changes (use with caution)
```

**Why absorb?** Keeps history clean. Small fixes belong in the commits they fix, not as separate "fix typo" commits.

## Example 4: Reorganizing Commit History

### Scenario A: Squashing Commits

**Situation:** Made 5 small WIP commits, want to combine into one logical commit.

```bash
# Before:
# c5: More tweaks
# c4: Fix another thing
# c3: Fix tests
# c2: Adjust logic
# c1: Initial implementation

# Squash all commits in branch
but squash bu

# Or squash specific range
but squash c2..c5    # Squashes c2, c3, c4, c5 into one

# Or squash specific commits
but squash c2 c3 c4    # Squashes these three
```

### Scenario B: Moving Files Between Commits

**Situation:** A file was committed in the wrong commit, need to move it.

```bash
# 1. See which files are in which commits
but status --json -f

# Output shows:
# c3: api.js, utils.js
# c2: config.js

# 2. Move utils.js from c3 to c2
but rub a2 c2    # File a2 (utils.js) → commit c2
```

### Scenario C: Moving Commit to Different Branch

**Situation:** Committed to wrong branch, need to move commit.

```bash
# 1. Check current state
but status --json

# Output:
# Branch: feature-a (bu)
#   c3: This should be in feature-b!
#   c2: Correct commit

# 2. Create or identify target branch
but branch new feature-b    # Creates branch bv

# 3. Move the commit
but move c3 bv    # Move c3 to top of branch bv
```

## Example 5: Using Marks for Focused Work

**Scenario:** Working on a large refactor, want all changes to automatically stage to that branch.

```bash
# 1. Create refactor branch
but branch new refactor

# 2. Mark it for auto-staging
but mark bu    # Branch bu (refactor) is now marked

# 3. Make changes across many files
# (edit 20 different files)

# 4. All changes automatically staged to refactor branch
but status --json  # Shows all changes staged to bu

# 5. Commit the staged changes
but commit bu --only -m "Refactor error handling across app"

# 6. Remove mark
but unmark
```

**Alternative: Mark a commit for auto-amend**

```bash
# 1. Create empty commit as placeholder
but commit empty -m "TODO: Add error handling"

# 2. Mark it
but mark c5    # Commit c5 is now marked

# 3. Make changes - they auto-amend into c5
# (edit files)

# 4. Check result
but show c5    # Shows accumulated changes

# 5. Remove mark when done
but unmark
```

## Example 6: Conflict Resolution

**Scenario:** After `but pull`, conflicts appear in a commit.

```bash
# 1. Pull updates
but pull

# Output:
# Conflict in commit c3 on branch feature-x

# 2. Check status
but status --json

# Output:
# Branch: feature-x (bu)
#   c3: Add validation (CONFLICTED)

# 3. Enter resolution mode
but resolve c3

# Output:
# Entering resolution mode for commit c3
# Fix conflicts in: api/users.js, api/validation.js

# 4. Edit files to resolve conflicts
# (open files, remove <<< === >>> markers)

# 5. Check progress
but resolve status

# Output:
# Remaining conflicts:
#   api/validation.js

# 6. Continue fixing...
# (resolve last conflict)

# 7. Finalize
but resolve finish

# Back to normal workspace mode
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

# 4. Check and stage
but status --json
but stage <file-ids> bu  # Stage changes to dashboard branch

# 5. First commit
but commit bu --only -m "Add dashboard route and basic layout"

# 6. Continue iterating
# (add widgets, styling)
but stage <file-ids> bu
but commit bu --only -m "Add dashboard widgets"
but stage <file-ids> bu
but commit bu --only -m "Style dashboard components"

# 7. Make small fix
# (fix typo in widget)
but status --json          # Check file ID of the fix (e.g., a1)
but absorb a1              # Absorb specific file into appropriate commit

# 8. Review history
but status --json

# 9. Clean up if needed
but squash bu    # Combine all commits (optional)

# 10. Push to remote (can also skip - pr new auto-pushes)
but push bu

# 11. Create pull request
but pr new bu

# Output:
# Created PR #123: https://github.com/org/repo/pull/123

# 12. After PR is merged, update
but pull
```

## Example 8: Working with Applied/Unapplied Branches

**Scenario:** Have 3 branches, but two are causing conflicts. Temporarily unapply them.

```bash
# 1. Check active branches
but status --json

# Output:
# Applied branches:
#   bu: feature-a
#   bv: feature-b
#   bw: feature-c

# 2. Conflicts between feature-b and feature-c
# Unapply them temporarily
but unapply bv
but unapply bw

# 3. Focus on feature-a
# (make changes, stage, commit)
but stage <file-ids> bu
but commit bu --only -m "Complete feature-a"

# 4. Create PR for feature-a (auto-pushes)
but pr new bu

# 5. Reapply other branches
but apply bv
but apply bw

# 6. Deal with their conflicts now
but resolve ...
```

## Example 9: Fixing History Before Pushing

**Scenario:** Made several commits, realized you need to reword messages and reorder.

```bash
# 1. Current state
but status --json

# Output:
# Branch: feature-x (bu)
#   c5: final commit
#   c4: WIP
#   c3: Fix stuff
#   c2: Another fix
#   c1: Initial

# 2. Reword commit messages
but reword c4 -m "Add validation logic"
but reword c3 -m "Fix edge case in parser"
but reword c2 -m "Update error messages"

# 3. Move c5 to be earlier
but move c5 c3    # Move c5 before c3

# 4. Squash similar commits
but squash c2 c3    # Combine error handling commits

# 5. Review final state
but status --json

# Output:
# Branch: feature-x (bu)
#   c4: Add validation logic
#   c3: final commit
#   c2: Fix edge case in parser and update error messages
#   c1: Initial

# 6. Push clean history
but push bu
```

## Example 10: Daily Development Workflow

**Typical day working with GitButler:**

```bash
# Morning: Start day
but pull                          # Get latest from team

# Start new task
but branch new fix-auth-bug       # Create branch for today's work

# Work and commit iteratively
# (make changes)
but status --json                 # Check changes
but stage <file-ids> bu           # Stage to branch
but commit bu --only -m "Identify auth bug source"
# (make more changes)
but stage <file-ids> bu           # Stage to branch
but commit bu --only -m "Fix token expiration handling"
# (small fix to existing code)
but status --json                 # Get file ID (e.g., a1)
but absorb a1                     # Absorb specific fix into appropriate commit

# Mid-day: Start urgent fix on different branch
but branch new hotfix-login       # Parallel branch for urgent work
# (make fix)
but stage <file-ids> bv           # Stage to hotfix branch
but commit bv --only -m "Fix login redirect loop"
but pr new bv                     # Push and create PR immediately

# Back to original work
# (continue working on bu, auth bug fix)
but stage <file-ids> bu           # Stage to auth branch
but commit bu --only -m "Add tests for token handling"

# End of day: Clean up and create PR
but squash bu                     # Combine into clean history
but pr new bu                     # Push and create PR

# After PR review: Make requested changes
# (make changes based on feedback)
but status --json                 # Check file IDs of changes
but absorb <file-id>              # Absorb specific changes into commits
# Or absorb all changes for this branch:
but absorb bu                     # Absorb all changes staged to bu
but push bu --with-force          # Force push updated history
```

## Example 11: Recovering from Mistakes

**Scenario:** Made changes you didn't mean to, need to undo.

### Undo Last Operation

```bash
# Made a mistake
but squash bu    # Oops! Didn't mean to squash

# Undo it
but undo         # Reverts the squash
```

### Restore to Earlier Point

```bash
# View operation history
but oplog

# Output:
# s5: squash branch bu
# s4: commit bu "message"
# s3: stage files to bu
# s2: create branch bu
# s1: pull from remote

# Restore to before squash
but oplog restore s4
```

### Discard Uncommitted Changes

```bash
# Changed a file but want to discard
but status --json

# Output:
# Unstaged:
#   a1: bad-changes.js

# Discard it
but discard a1
```

## Tips and Tricks

### Quick Status Check

```bash
but status -f    # File-centric view for quick overview
```

### Preview Before Doing

```bash
but absorb <file-id> --dry-run  # See where specific file would be absorbed
but push --dry-run      # See what would be pushed
```

### JSON for Scripting

```bash
but status --json | jq '.branches[] | .name'    # List branch names
```

### Auto-completion

```bash
eval "$(but completions zsh)"     # Add to ~/.zshrc
eval "$(but completions bash)"    # Add to ~/.bashrc
```

### Viewing History

```bash
but show bu              # Show all commits in branch
git log bu               # Traditional git log (read-only, still works)
```
