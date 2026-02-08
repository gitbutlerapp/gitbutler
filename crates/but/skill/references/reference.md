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

**Hunk IDs in JSON output:** For uncommitted changes, `but diff --json` returns each hunk as a separate entry in `changes[]` with an `id` field (e.g., `e8`, `j0`). Pass these IDs to `but commit --changes` for fine-grained, hunk-level commits. For commit/branch diffs, `id` is absent — entries are per-file with hunks nested under `diff.hunks`.

## Branching

### `but branch`

List all branches (default when no subcommand).

```bash
but branch              # List branches
but branch --json       # JSON output
but branch list [filter]  # Filter branches by name (case-insensitive substring)
but branch list --no-ahead  # Skip commits-ahead calculation (faster)
but branch list --no-check  # Skip clean-merge check (faster)
but branch list -r      # Show only remote branches
but branch list -l      # Show only local branches
but branch list -a      # Show all branches (not just active + 20 most recent)
but branch list --review  # Fetch and display PR/MR information
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
but branch show <id> -f       # Show files modified in each commit with line counts
but branch show <id> --ai     # Generate AI summary of branch changes
but branch show <id> --check  # Check if branch merges cleanly into upstream
but branch show <id> -r       # Fetch and display review information (PRs/MRs)
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
but stage <file-id> <branch-id> --status-after  # Stage then show workspace status
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
but commit <branch> -m "message" --changes <id> --changes <id>  # Alternative: repeat flag
but commit <branch> --message-file msg.txt  # Read commit message from file
but commit <branch> -i                   # AI-generated commit message
but commit <branch> -i="fix the auth bug"  # AI-generated with instructions (equals sign required)
but commit <branch> -m "message" --status-after  # Commit then show workspace status
but commit <branch> -c -m "message"      # Create new branch (or use existing) and commit
but commit <branch> -n -m "message"      # Bypass git commit hooks (pre-commit, commit-msg, post-commit)
but commit empty --before <target>       # Insert empty commit before target
but commit empty --after <target>        # Insert empty commit after target
```

**Important:** Without `--only`, ALL uncommitted changes are committed to the branch, not just staged files. Use `--only` when you've staged specific files and want to commit only those.

**Committing specific files or hunks:** Use `--changes` (or `-p`) with comma-separated CLI IDs to commit only those files or hunks:
- **File IDs** from `but status --json`: commits entire files
- **Hunk IDs** from `but diff --json`: commits individual hunks
- `--changes` takes one argument per flag. Use `--changes a1,b2` or `--changes a1 --changes b2`, not `--changes a1 b2`.

**Note:** `--changes` and `--only` are mutually exclusive.

**AI commit messages:** Use `-i` / `--ai` by itself for auto-generated messages, or `--ai="your instructions"` (equals sign required) to provide guidance.

**Creating branches on commit:** Use `-c` / `--create` to create a new branch for the commit. If the branch name matches an existing branch, that branch is used instead.

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
but absorb --new/-n           # Create new commits instead of amending existing ones
but absorb --status-after     # Absorb then show workspace status
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
but rub <file> zz            # Unstage file (back to unassigned)
but rub <commit> zz          # Undo commit (uncommit to unstaged)
but rub zz <branch>          # Stage all unassigned changes to branch
but rub zz <commit>          # Amend all unassigned changes into commit
but rub <file-in-commit> zz  # Uncommit specific file from its commit
but rub <file-in-commit> <commit>  # Move file from one commit to another
but rub <branch> <branch>    # Reassign all uncommitted changes between branches
but rub <file> <commit> --status-after  # Amend then show workspace status
```

The core "rub two things together" operation.

**Full operations matrix:**

```
SOURCE ↓ / TARGET →  │ zz (unassigned) │ Commit     │ Branch      │ Stack
─────────────────────┼─────────────────┼────────────┼─────────────┼────────────
File/Hunk            │ Unstage         │ Amend      │ Stage       │ Stage
Commit               │ Undo            │ Squash     │ Move        │ -
Branch (all changes) │ Unstage all     │ Amend all  │ Reassign    │ Reassign
Stack (all changes)  │ Unstage all     │ -          │ Reassign    │ Reassign
Unassigned (zz)      │ -               │ Amend all  │ Stage all   │ Stage all
File-in-Commit       │ Uncommit        │ Move       │ Uncommit to │ -
```

`zz` is a special target meaning "unassigned" (no branch).

### `but squash <commits>`

Squash commits together.

```bash
but squash <c1> <c2> <c3>    # Squash multiple commits (into last)
but squash <c1>..<c4>        # Squash a range
but squash <branch>          # Squash all commits in branch into bottom-most
but squash <branch> -d       # Squash and drop source commit messages (keep target's)
but squash <branch> -m "msg" # Squash with a new commit message
but squash <branch> -i       # Squash with AI-generated commit message
but squash <branch> --status-after  # Squash then show workspace status
```

### `but amend <file> <commit>`

Amend file into a specific commit. Use when you know exactly which commit the change belongs to.

```bash
but amend <file-id> <commit-id>                  # Amend file into specific commit
but amend <file-id> <commit-id> --status-after   # Amend then show workspace status
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
but move <commit> <branch> --status-after  # Move then show workspace status
```

### `but uncommit <source>`

Uncommit changes back to unstaged area.

```bash
but uncommit <commit-id>      # Uncommit entire commit
but uncommit <file-id>        # Uncommit specific file from its commit
but uncommit <commit-id> --status-after  # Uncommit then show workspace status
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
but push -s                   # Skip force push protection checks
but push -r                   # Run pre-push hooks
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
but pr new <branch-id> -t     # Use default content (commit message), skip prompts
but pr                        # Create PR (prompts for branch)
but pr template               # Configure PR description template
```

**Key behavior:** `but pr new` automatically pushes the branch to remote before creating the PR. No need to run `but push` first. Force push (`--with-force`) and pre-push hooks (`--run-hooks`) are enabled by default.

In non-interactive environments, use `--message (-m)`, `--file (-F)`, or `--default (-t)` to avoid editor prompts. The `-t` flag uses the commit message as title/description for single-commit branches; for multi-commit branches it falls back to the branch name as the title.

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
but setup --init              # Also initialize a new git repo if none exists
```

Converts regular git repo to use GitButler workspace model. Use `--init` in non-interactive environments (CI/CD) to ensure a git repository exists before setup.

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
- `--status-after` - After a mutation command, also output workspace status. In human mode, prints status after the command output. In JSON mode, wraps both in `{"result": ..., "status": ...}` on success, or `{"result": ..., "status_error": "..."}` if the status query fails. Supported on: `rub`, `commit`, `stage`, `amend`, `absorb`, `squash`, `move`, `uncommit`.
- `-C, --current-dir <PATH>` - Run as if started in different directory
- `-h, --help` - Show help for command

## Getting More Help

```bash
but --help                    # List all commands
but <subcommand> --help       # Detailed help for specific command
```

Full documentation: <https://docs.gitbutler.com/cli-overview>
