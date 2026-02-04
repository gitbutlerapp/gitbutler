# GitButler CLI Command Reference

Comprehensive reference for all `but` commands.

## Contents

- [Inspection](#inspection-understanding-state) - `status`, `show`, `diff`
- [Branching](#branching) - `branch new`, `apply`, `unapply`, `branch delete`, `pick`
- [Staging](#staging-multiple-staging-areas) - `stage`, `rub`
- [Committing](#committing) - `commit`, `absorb`
- [Editing History](#editing-history) - `rub`, `squash`, `amend`, `move`, `uncommit`, `reword`, `discard`
- [Conflict Resolution](#conflict-resolution) - `resolve`
- [Remote Operations](#remote-operations) - `push`, `pull`, `pr`, `merge`
- [Automation](#automation) - `mark`, `unmark`
- [History & Undo](#history--undo) - `undo`, `oplog`
- [Setup & Configuration](#setup--configuration) - `setup`, `teardown`, `config`, `gui`, `update`, `alias`
- [Global Options](#global-options)

## Inspection (Understanding State)

### `but status`

Overview of workspace state - this is your entry point.

```bash
but status              # Human-readable view
but status -f           # File-centric view (shows which files in which commits)
but status --json       # Structured output for parsing
but status --verbose    # Detailed information
but status --upstream   # Show upstream relationship
```

Shows:

- Applied/unapplied branches in workspace
- Uncommitted changes (unstaged files)
- Commits on each stack
- CLI IDs to use in other commands

### `but show <id>`

Details about a commit or branch.

```bash
but show <id>           # Show details
but show <id> --verbose # More detailed information
but show <id> --json    # Structured output
```

### `but diff [target]`

Display diff for file, branch, stack, or commit.

```bash
but diff <file-id>      # Diff for specific file
but diff <branch-id>    # Diff for all changes in branch
but diff <commit-id>    # Diff for specific commit
but diff                # Diff for entire workspace
but diff --json         # JSON output with hunk IDs for `but commit --changes`
```

## Branching

### `but branch`

List all branches (default when no subcommand).

```bash
but branch              # List branches
but branch --json       # JSON output
```

### `but branch new <name>`

Create a new branch.

```bash
but branch new feature              # Independent branch (parallel work)
but branch new feature -a <anchor>  # Stacked branch (dependent work)
```

Use parallel branches for independent tasks. Use stacked branches when work depends on another branch.

### `but apply <id>`

Activate a branch in the workspace.

```bash
but apply <id>           # Activate branch in workspace
```

Applied branches are merged into `gitbutler/workspace` and visible in working directory.

### `but unapply <id>`

Deactivate a branch from the workspace.

```bash
but unapply <id>         # Deactivate branch from workspace
```

The identifier can be a CLI ID pointing to a stack or branch, or a branch name. If a branch is specified, the entire stack containing that branch will be unapplied.

### `but branch delete <id>`

Delete a branch.

```bash
but branch delete <id>
but branch -d <id>      # Short form
```

### `but branch show <id>`

Show commits ahead of base for a branch.

```bash
but branch show <id>
```

### `but pick <source> [target]`

Cherry-pick commits from unapplied branches into applied branches.

```bash
but pick <commit-sha> <branch>       # Pick specific commit into branch
but pick <cli-id> <branch>           # Pick using CLI ID (e.g., "c5")
but pick <unapplied-branch>          # Interactive commit selection from branch
but pick <commit-sha>                # Auto-select target if only one branch
```

The source can be:
- A commit SHA (full or short)
- A CLI ID from `but status`
- An unapplied branch name (shows interactive commit picker)

If no target is specified and multiple branches exist, prompts for selection interactively.

## Staging (Multiple Staging Areas)

GitButler has multiple staging areas - one per stack.

### `but stage <file> <branch>`

Stage file to a specific branch.

```bash
but stage <file-id> <branch-id>
```

Alias for `but rub <file> <branch>`. You can't stage changes that depend on branch A to branch B.

### `but rub <file> <branch>`

Core primitive for staging (see Editing History for other `but rub` uses).

```bash
but rub <file-id> <branch-id>    # Stage file to branch
```

## Committing

### `but commit [branch]`

Commit changes to a branch.

```bash
but commit <branch> --only -m "message"  # Commit ONLY staged changes (recommended)
but commit <branch> -m "message"         # Commit ALL uncommitted changes to branch
but commit <branch> -m "message" --changes <id>,<id>  # Commit specific files or hunks by CLI ID
but commit <branch> -i                   # AI-generated commit message
but commit empty --before <target>       # Insert empty commit before target
but commit empty --after <target>        # Insert empty commit after target
```

**Important:** Without `--only`, ALL uncommitted changes are committed to the branch, not just staged files. Use `--only` when you've staged specific files and want to commit only those.

**Committing specific files or hunks:** Use `--changes` (or `-p`) with comma-separated CLI IDs to commit only those files or hunks:
- **File IDs** from `but status --json`: commits entire files
- **Hunk IDs** from `but diff --json`: commits individual hunks

**Note:** `--changes` and `--only` are mutually exclusive.

Example: `but commit my-branch -m "Fix bug" --changes ab,cd` commits files/hunks `ab` and `cd`.

To commit specific hunks from a file with multiple changes, use `but diff --json` to see hunk IDs, then specify them individually.

If only one branch is applied, you can omit the branch ID.

### `but absorb [source]`

Automatically amend uncommitted changes into existing commits.

```bash
but absorb <file-id>          # Absorb specific file (recommended)
but absorb <branch-id>        # Absorb all changes staged to this branch
but absorb                    # Absorb ALL uncommitted changes (use with caution)
but absorb --dry-run          # Preview without making changes
but absorb <file-id> --dry-run  # Preview specific file absorption
```

**Recommendation:** Prefer targeted absorb (`but absorb <file-id>`) over absorbing everything. Running `but absorb` without arguments absorbs ALL uncommitted changes across all branches, which may not be what you want.

Logic:

- Changes amended into topmost commit of their branch
- Changes depending on specific commit amended into that commit
- Uses smart matching to find appropriate commits

## Editing History

### `but rub <source> <dest>`

Universal editing primitive that does different operations based on types.

```bash
but rub <file> <branch>      # Stage file to branch
but rub <file> <commit>      # Amend file into commit
but rub <commit> <commit>    # Squash commits together
but rub <commit> <branch>    # Move commit to branch
```

The core "rub two things together" operation.

### `but squash <commits>`

Squash commits together.

```bash
but squash <c1> <c2> <c3>    # Squash multiple commits (into last)
but squash <c1>..<c4>        # Squash a range
but squash <branch>          # Squash all commits in branch into bottom-most
```

### `but amend <file> <commit>`

Amend file into a specific commit. Use when you know exactly which commit the change belongs to.

```bash
but amend <file-id> <commit-id>    # Amend file into specific commit
```

**When to use `amend` vs `absorb`:**
- `but amend` - You know the target commit; explicit control
- `but absorb` - Let GitButler auto-detect the target; smart matching based on dependencies

Alias for `but rub <file> <commit>`.

### `but move <commit> <target>`

Move a commit to a different location.

```bash
but move <source> <target>           # Move before target
but move <source> <target> --after   # Move after target
but move <commit> <branch>           # Move to top of branch
```

### `but uncommit <source>`

Uncommit changes back to unstaged area.

```bash
but uncommit <commit-id>      # Uncommit entire commit
but uncommit <file-id>        # Uncommit specific file from its commit
```

### `but reword <id>`

Reword commit message or rename branch.

```bash
but reword <id>               # Interactive editor
but reword <id> -m "new"      # Non-interactive
but reword <id> --format      # Format to 72-char wrapping
```

### `but discard <id>`

Discard uncommitted changes.

```bash
but discard <file-id>         # Discard file changes
but discard <hunk-id>         # Discard hunk changes
```

## Conflict Resolution

When commits have conflicts (shown in `but status`):

### `but resolve <commit>`

Enter resolution mode for a conflicted commit.

```bash
but resolve <commit-id>
```

### `but resolve status`

Show remaining conflicted files.

```bash
but resolve status
```

### `but resolve finish`

Finalize conflict resolution.

```bash
but resolve finish
```

### `but resolve cancel`

Cancel conflict resolution and return to workspace mode.

```bash
but resolve cancel
```

**Workflow:**

1. `but resolve <commit>` - Enter mode
2. Edit files to resolve conflicts
3. `but resolve status` - Check progress
4. `but resolve finish` - Complete

## Remote Operations

### `but push [branch]`

Push branches to remote.

```bash
but push                      # Push all branches with unpushed commits
but push <branch-id>          # Push specific branch
but push --dry-run            # Preview what would be pushed
but push --with-force         # Force push (use carefully!)
```

### `but pull`

Update all branches with target branch changes.

```bash
but pull                      # Fetch and rebase all branches
but pull --check              # Check if can merge cleanly (no changes)
```

Run regularly to stay up to date with main development line.

### `but pr`

Create and manage pull requests.

```bash
but pr new <branch-id>        # Push branch and create PR (recommended)
but pr new <branch-id> -F pr_message.txt    # Use file: first line is title, rest is description
but pr new <branch-id> -m "Title..."        # Inline message: first line is title, rest is description
but pr                        # Create PR (prompts for branch)
but pr template               # Configure PR description template
```

**Key behavior:** `but pr new` automatically pushes the branch to remote before creating the PR. No need to run `but push` first.

In non-interactive environments, use `--message (-m)`, `--file (-F)`, or `--default (-t)` to avoid editor prompts.

**Note:** For stacked branches, the custom message (`-m` or `-F`) only applies to the selected branch. Dependent branches in the stack will use default messages (commit title/description).

Requires forge integration to be configured via `but config forge auth`.

### `but merge <branch>`

Merge branch into local target branch.

```bash
but merge <branch-id>
```

Merges into local target branch, then runs `but pull` to update.

## Automation

### `but mark <target>`

Auto-stage or auto-commit new changes.

```bash
but mark <branch-id>          # New unstaged changes auto-stage to this branch
but mark <commit-id>          # New changes auto-amend into this commit
but mark <id> --delete        # Remove the mark
```

### `but unmark`

Remove all marks.

```bash
but unmark
```

Use marks when working on a focused area to automatically organize changes.

## History & Undo

### `but undo`

Undo last operation.

```bash
but undo
```

Reverts to previous oplog snapshot.

### `but oplog`

View operation history.

```bash
but oplog
but oplog --json
```

Shows all operations with snapshot IDs.

### `but oplog restore <snapshot>`

Restore to a specific oplog snapshot.

```bash
but oplog restore <snapshot-id>
```

## Setup & Configuration

### `but setup`

Initialize GitButler in current git repository.

```bash
but setup
```

Converts regular git repo to use GitButler workspace model.

### `but teardown`

Exit GitButler mode and return to normal git workflow.

```bash
but teardown
```

### `but config`

View and manage GitButler configuration.

```bash
but config
```

### `but gui`

Open GitButler GUI for current project.

```bash
but gui
```

### `but update`

Manage GitButler CLI and app updates.

```bash
but update
```

### `but alias`

Manage command aliases.

```bash
but alias
```

## Global Options

Available on most commands:

- `-j, --json` - Output in JSON format for parsing
- `-C, --current-dir <PATH>` - Run as if started in different directory
- `-h, --help` - Show help for command

## Getting More Help

```bash
but --help                    # List all commands
but <subcommand> --help       # Detailed help for specific command
```

Full documentation: <https://docs.gitbutler.com/cli-overview>
