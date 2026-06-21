# GitButler CLI Command Reference

Agent-focused reference for useful `but` commands.

## Contents

- [Inspection](#inspection-understanding-state) - `status`, `show`, `diff`
- [Branching](#branching) - `branch new`, `apply`, `unapply`, `branch delete`, `pick`
- [Committing](#committing) - `commit`, `absorb`
- [Editing History](#editing-history) - `rub`, `squash`, `amend`, `move`, `uncommit`, `reword`, `discard`
- [Conflict Resolution](#conflict-resolution) - `resolve`
- [Remote Operations](#remote-operations) - `push`, `pull`, `pr`, `merge`
- [Workspace Maintenance](#workspace-maintenance) - `clean`
- [History & Undo](#history--undo) - `undo`, `oplog`
- [Setup & Configuration](#setup--configuration) - `setup`, `teardown`, `config`, `update`, `skill`, `gui`
- [Selected Options](#selected-options)

## Inspection (Understanding State)

### `but status`

Overview of branch, stack, commit, and workspace state. Use this when you need existing branch/stack/commit/conflict context. For selected dirty-file or hunk commits, start with `but diff` instead.

```bash
but status              # Compact human overview; avoid as routine preflight for write tasks
but status -fv          # File-centric view with full commit details
but status --verbose    # Detailed information
but status --upstream   # Show upstream relationship
```

Shows:

- Applied/unapplied branches in workspace
- Unassigned and assigned changes
- Commits on each stack
- CLI IDs to use in other commands

### `but show <id>`

Details about a commit or branch.

```bash
but show <id>           # Show details
but show <id> --verbose # Show with full messages and file details
```

### `but diff [target]`

Display diff for file, branch, stack, or commit.

```bash
but diff                # Diff for entire workspace; best first command for selective dirty commits
but diff <file-id>      # Diff for specific file
but diff <branch-id>    # Diff for all changes in branch
but diff <commit-id>    # Diff for specific commit
```

**Hunk IDs:** For uncommitted changes, `but diff` shows each hunk with an ID (e.g., `e8`, `j0`). Pass these IDs to `but commit --changes` for fine-grained, hunk-level commits.

## Branching

### `but branch`

List all branches (default when no subcommand).

```bash
but branch              # List branches
but branch list [filter]  # Filter branches by name (case-insensitive substring)
but branch list --no-ahead  # Skip commits-ahead calculation (faster)
but branch list --no-check  # Skip clean-merge check (faster)
but branch list -r      # Show only remote branches
but branch list -l      # Show only local branches
but branch list -a      # Show all branches (not just active + 20 most recent)
but branch list --empty  # Include empty branches
but branch list --review  # Fetch and display review information
```

### `but branch new [name]`

Create a new branch.

```bash
but branch new                      # Generated branch name
but branch new feature              # Independent branch (parallel work)
but branch new feature -a <anchor>  # Stacked branch (dependent work)
```

Use parallel branches for independent tasks. Use stacked branches when work depends on another branch.

For "commit these selected changes on a new branch", prefer `but commit <branch> -c -m "message" --changes <ids>` instead of a separate `but branch new` or preflight `but status -fv`.

### `but apply <branch-name>`

Activate a branch in the workspace.

```bash
but apply feature-branch  # Activate branch in workspace
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
but branch show <id> -r       # Fetch and display review information
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

## Committing

### `but commit [branch]`

Commit changes to a branch.

```bash
but commit <branch> -m "message"         # Commit ALL uncommitted changes to branch
but commit <branch> -am "message"        # Accepted Git muscle-memory form; -a is a no-op
but commit <branch> -m "message" --changes <id>,<id>  # Commit specific files or hunks by CLI ID
but commit <branch> -m "message" --changes <id> --changes <id>  # Alternative: repeat flag
but commit <branch> --message-file msg.txt  # Read commit message from file
but commit <branch> -c -m "message"      # Create new branch (or use existing) and commit
but commit <branch> -n -m "message"      # Bypass git commit hooks (pre-commit, commit-msg, post-commit)
but commit empty                         # Insert empty commit at top of first branch
but commit empty -m "message"            # Insert empty commit with message
but commit empty <target>                # Insert empty commit before target
but commit empty --before <target>       # Insert empty commit before target
but commit empty --after <target>        # Insert empty commit after target
```

**Important:** Plain `but commit <branch> -m` commits ALL uncommitted changes to the branch. Use `--changes` to commit only specific files or hunks.

**Committing specific files or hunks:** Start with `but diff` for selective dirty commits, then use `--changes` (or `-p`) with comma-separated CLI IDs to commit only those files or hunks:
- **File IDs** from `but diff` or `but status -fv`: commits entire files
- **Hunk IDs** from `but diff`: commits individual hunks
- `--changes` takes one argument per flag. Use `--changes a1,b2` or `--changes a1 --changes b2`, not `--changes a1 b2`.

**Creating branches on commit:** Use `-c` / `--create` to create a new branch for the commit. If the branch name matches an existing branch, that branch is used instead.

Example: `but commit my-branch -m "Fix bug" --changes ab,cd` commits files/hunks `ab` and `cd`.

Example new branch: `but commit feature/contact-form -c -m "Validate contact form input" --changes ab,cd` creates `feature/contact-form` and commits only those selected file or hunk IDs.

To commit specific hunks from a file with multiple changes, use `but diff` to see hunk IDs, then specify them individually.

Edge case: if wanted and unwanted edits are in the same hunk, GitButler cannot split that hunk by ID. Only when the task requires keeping part of that hunk uncommitted, temporarily edit the working tree to isolate the wanted lines, commit with `--changes`, then restore the leftover lines so they remain uncommitted.

If only one branch is applied, you can omit the branch ID.

### `but absorb [source]`

Automatically amend uncommitted changes into existing commits.

```bash
but absorb <file-id>          # Absorb specific file (recommended)
but absorb <branch-id>        # Absorb all changes assigned to this branch
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
but rub <file> <commit>      # Amend file into commit
but rub <commit> <commit>    # Squash commits together
but rub <commit> <branch>    # Move commit to branch
but rub <commit> zz          # Undo commit to unassigned
but rub zz <commit>          # Amend all unassigned changes into commit
but rub <file-in-commit> zz  # Uncommit specific file from its commit
but rub <file-in-commit> <commit>  # Move file from one commit to another
```

The core "rub two things together" operation. `zz` is a special target meaning "unassigned" (no branch).

### `but squash <commits>`

Squash commits together.

```bash
but squash <c1> <c2> <c3>    # Squash multiple commits (into last)
but squash <c1>..<c4>        # Squash a range
but squash <branch>          # Squash all commits in branch into bottom-most
but squash <branch> -d       # Squash and drop source commit messages (keep target's)
but squash <branch> -m "msg" # Squash with a new commit message
but squash <branch> -i       # Squash with AI-generated commit message
```

### `but amend <commit> --changes <file>[,<file>...]`

Amend one or more files/hunks into a specific commit. Use when you know exactly which commit the change belongs to.

```bash
but amend <commit-id> --changes <file-id>,<hunk-id>
```

**When to use `amend` vs `absorb`:**
- `but amend` - You know the target commit; explicit control
- `but absorb` - Let GitButler auto-detect the target; smart matching based on dependencies

Convenience wrapper around `rub` for amending uncommitted files or hunks into a known commit.

### `but move <source> <target>`

Move commits or branches to a different location.

```bash
but move <commit> <target-commit>            # Move before target commit
but move <commit>,<commit> <target-commit>   # Move multiple commits before target
but move <commit> <target-commit> --after    # Move after target commit
but move <commit> <branch>                   # Move commit to top of branch
but move <branch> <target-branch>            # Stack branch on top of target branch
but move <branch> zz                          # Tear off (unstack) branch
```

`--after` is valid only for commit-to-commit moves.

### `but uncommit <source>`

Uncommit changes back to unassigned changes.

```bash
but uncommit <commit-id>      # Uncommit entire commit
but uncommit <file-id>        # Uncommit specific file from its commit
but uncommit <commit-id> --diff  # Also show resulting dirty diff with hunk IDs
but uncommit <commit-id> -d   # Discard committed changes instead of moving to unassigned
but uncommit <file-id> --discard  # Discard committed file changes completely
```

Use `--diff` when you plan to recommit selected files or hunks immediately after uncommitting.

### `but reword <id>`

Reword commit message or rename branch.

```bash
but reword <id>               # Interactive editor
but reword <id> -m "new"      # Non-interactive
but reword <id> --fix-formatting  # Format to 72-char wrapping
```

### `but discard <id>`

Discard uncommitted changes.

```bash
but discard <file-id>         # Discard file changes
but discard <hunk-id>         # Discard hunk changes
```

## Conflict Resolution

When commits have conflicts (shown in `but status` — look for commits marked as conflicted):

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
but resolve cancel --force
```

**Workflow:**

1. `but status` — identify conflicted commits (marked as conflicted in the output)
2. `but resolve <commit-id>` — enter resolution mode for the conflicted commit
3. Edit the conflicted files — remove `<<<<<<<`, `=======`, `>>>>>>>` markers and keep the correct content
4. `but resolve status` — verify no conflicts remain
5. `but resolve finish` — finalize and return to normal mode
6. If multiple commits are conflicted, repeat steps 2-5 for each one

**Important:** Never use `git add`, `git commit`, or other git write commands during conflict resolution. Only use `but resolve` commands and edit files directly.

## Remote Operations

### `but push <branch>`

Push a branch to remote. Always specify which branch to push: without one, `but push` prompts for a selection in interactive terminals and pushes ALL branches with unpushed commits otherwise. Accepts a full branch name or a branch CLI ID — prefer the name; it stays valid across mutations.

```bash
but push <branch-name>             # Push specific branch
but push <branch-name> --dry-run   # Preview what would be pushed
but push <branch-name> -s          # Skip force push protection checks
but push <branch-name> --no-hooks  # Bypass pre-push hooks (--no-verify also works)
```

Force push is enabled by default with protection checks. Use `-s` only when intentionally skipping those checks.

### `but pull`

Update applied branches onto the latest target branch changes (usually `main`).
Use this for "get latest from main" in a GitButler workspace.

```bash
but pull                      # Fetch and rebase applied branches
but pull --check              # Check if can merge cleanly (no changes)
```

Run `but pull --check` first, then `but pull` if clean. Do not use raw
`git pull` or `git rebase`.

### `but pr`

Create and manage pull requests.

```bash
but pr new <branch-id>        # Push branch and create PR (recommended)
but pr new <branch-id> -F pr_message.txt    # Use file: first line is title, rest is description
but pr new <branch-id> -m "Title..."        # Inline message: first line is title, rest is description
but pr new <branch-id> -t     # Use default content (commit message), skip prompts
but pr new <branch-id> --draft  # Create as draft
but pr new <branch-id> --no-hooks  # Bypass pre-push hooks (--no-verify also works)
but pr new <branch-id> -s     # Skip force-push protection checks
but pr --draft                # Top-level draft flag
but pr auto-merge <selector>  # Enable auto-merge
but pr set-draft <selector>   # Mark review as draft
but pr set-ready <selector>   # Mark review as ready
```

**Key behavior:** `but pr new` automatically pushes the branch to remote before creating the PR. No need to run `but push` first. Force push and pre-push hooks run by default.
Use `--no-hooks` to bypass pre-push hooks when needed.

Selectors for `auto-merge`, `set-draft`, and `set-ready` can be branch names, branch IDs, stack IDs, or numeric review IDs, comma-separated.

In non-interactive environments, use `--message (-m)`, `--file (-F)`, or `--default (-t)` to avoid editor prompts. The `-t` flag uses the commit message as title/description for single-commit branches; for multi-commit branches it falls back to the branch name as the title.

**Note:** For stacked branches, the custom message (`-m` or `-F`) only applies to the selected branch. Dependent branches in the stack will use default messages (commit title/description).

Requires forge integration to be configured via `but config forge auth`.

### `but merge <branch>`

Merge branch into local target branch.

```bash
but merge <branch-id>
```

Merges into local target branch, then runs `but pull` to update.

## Workspace Maintenance

### `but clean`

Remove empty branches from the workspace.

```bash
but clean                   # Delete all empty branches
but clean --dry-run         # Preview which branches would be deleted
but clean --pull            # Pull latest changes first, then clean
but clean --include-upstream # Also remove branches with upstream-only commits
```

A branch is considered empty if it has no local commits and no assigned changes. Branches with upstream-only commits are preserved by default unless `--include-upstream` is used.

The entire operation is a single oplog entry — use `but undo` to restore all deleted branches.

## History & Undo

### `but undo` / `but redo`

Undo or redo operations.

```bash
but undo
but redo
```

### `but oplog`

View operation history.

```bash
but oplog
but oplog list --since <snapshot-id>
but oplog list --snapshot
but oplog snapshot -m "known good"
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
but config user               # Also: forge, target, metrics, ui, ai
but config ai openai          # Also: anthropic, ollama, lmstudio, openrouter
```

### `but update check`

Manage GitButler CLI and app updates.

```bash
but update check
but update install
but update install [nightly|release|0.18.7]
```

### `but skill`

Manage installed GitButler skill files.

```bash
but skill check
but skill check --update
but skill install --detect
```

### `but gui [path]`

Open the GitButler desktop app for a project directory.

```bash
but gui                     # Open the current directory in the app
but gui ../other-repo       # Open a specific project directory
but gui --new-window        # Open the current project in a new app window
but gui -n ../other-repo    # Short flag for opening another project in a new window
```

## Selected Options

Useful to agents:

- `-C, --current-dir <PATH>` - Run as if started in different directory
- `-h, --help` - Show help for command. Avoid routine help probes; use this reference first.

## External commands (PATH helpers)

> Important: Not available for Windows yet

Similar to Git, if `<command>` is not a built-in `but` command and `but-<command>` exists on `PATH`, `but` runs that executable instead (for example `but forecast …` invokes `but-forecast …`).

Restriction: `<command>` must consist of characters in the set `[a-zA-Z_-]`

## Getting More Help

```bash
but --help                    # List all commands
but <subcommand> --help       # Detailed help for specific command
```

Use help only after a command fails or the installed references do not contain the syntax you need.

Full documentation: <https://docs.gitbutler.com/cli-overview>
