# GitButler CLI Command Reference

Agent-focused reference for useful `but` commands.

## Contents

- [Inspection](#inspection-understanding-state) - `status`, `show`, `diff`, `open`
- [Branching](#branching) - `branch new`, `apply`, `unapply`, `branch delete`, `pick`
- [Committing](#committing) - `commit`
- [Editing History](#editing-history) - `squash`, `amend`, `move`, `uncommit`, `reword`, `discard`
- [Conflict Resolution](#conflict-resolution) - `resolve`
- [Remote Operations](#remote-operations) - `push`, `pull`, `pr`, `land`
- [Workspace Maintenance](#workspace-maintenance) - `clean`, `worktree`
- [History & Undo](#history--undo) - `undo`, `oplog`
- [Setup & Configuration](#setup--configuration) - `setup`, `teardown`, `config`, `update`, `skill`
- [Selected Options](#selected-options)

## Inspection (Understanding State)

### `but status`

Overview of branch, stack, commit, and workspace state. Use this when you need existing branch/stack/commit/conflict context. For selected dirty-file or hunk commits, start with `but diff` instead.

```bash
but status              # Compact overview with branch, stack, commit IDs, and commit subjects
but status -fv          # File-centric view with full commit details and file IDs
but status --verbose    # Detailed information
but status --upstream   # Show upstream relationship
```

Shows:

- Applied/unapplied branches in workspace
- Uncommitted and assigned changes
- Commits on each stack
- CLI IDs to use in other commands

The first token on each line is that line's ID. A `<branch-selector>` is a full branch name or short ID from the current workspace snapshot. Short IDs may be reassigned as that context changes; branch names remain stable across unrelated workspace mutations. Agents should use full branch names for branch-targeting mutations. Commit lines lead with the commit's change ID (stable across history edits); commits without a change ID lead with a sha prefix, which goes stale after history edits. Verbose output appends an informational `(sha …)` after the timestamp — do not pass the sha to commands.

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

`but diff` accepts at most one target. Bare `but diff` shows every uncommitted file;
inspect committed files or other entities one target at a time. Unlike `commit`,
`amend`, and `discard`, it does not accept several positional IDs.

**Hunk IDs:** For uncommitted changes, `but diff` shows each hunk with an ID (e.g., `qs:5`, `uo:d`). Pass these IDs to `but commit` for fine-grained, hunk-level commits.

For the full CLI ID model, `but help cli-ids` documents every ID kind and its stability.

### `but open [target]`

Open the GitButler app at a branch or commit, or print the link with `--print`.

```bash
but open                    # The workspace
but open <branch-id>        # With that branch selected
but open <commit-id>        # With that commit selected
but open --print <id>       # Print the link instead of opening it
```

Commits are addressed by change ID where they have one, so the link keeps
working after the commit is amended or rebased.

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

To rename an applied branch, use `but reword <branch> -m "new-name"` (unapplied branches cannot be renamed).

### `but branch new [name]`

Create a new branch.

```bash
but branch new                      # Generated branch name
but branch new feature              # Independent branch (parallel work)
but branch new feature -a <anchor>  # Stacked branch (dependent work)
```

Use parallel branches for independent tasks. Use stacked branches when work depends on another branch.

In single-branch mode (no managed workspace), `but branch new` stacks the new branch above the checked-out branch (or the `-a` anchor). When the new branch lands above the checked-out branch — always the case without an anchor — it is checked out and `HEAD` moves to it.

For "commit these selected changes on a new branch", prefer `but commit -b <branch> -m "message" <ids>` instead of a separate `but branch new` or preflight `but status -fv` — `-b` creates the branch when it does not exist.

### `but apply <branch-name>`

Activate a branch in the workspace.

```bash
but apply feature-branch  # Activate branch in workspace
```

Default human output reports whether the branch was applied, was already active, or conflicted. Conflicts are reported as non-zero CLI errors.

### `but unapply <selector>`

Deactivate a branch from the workspace.

```bash
but unapply <selector> # Deactivate branch from workspace
```

The command also accepts a current CLI ID pointing to a stack or branch, but agents should use the full branch name. The entire stack containing that branch will be unapplied.

### `but branch delete <branch-selector>`

Delete a branch.

```bash
but branch delete <branch-selector>
but branch -d <branch-selector>      # Short form
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

### `but pick [SOURCES]...`

Cherry-pick commits from unapplied branches into applied branches.

```bash
but pick <commit-sha> --branch <branch>       # Pick specific commit into branch
but pick <cli-id> --branch <branch>           # Pick using CLI ID (e.g., "nn")
```

Name both the source commit and the target branch. Omitting the target prompts
for one when several branches exist. The source can be a commit SHA (full or
short) or a CLI ID from `but status`.

## Committing

### `but commit [CHANGES]...`

Create a commit. Changes are positional CLI IDs; where the commit goes is a flag.

```bash
but commit -b <branch> -m "message"          # Commit ALL uncommitted changes to branch
but commit -b <branch> -m "message" <id> <id>  # Commit specific files or hunks by CLI ID
but commit -b <branch> -m "msg" -m "body"    # Repeat -m; parts joined by a blank line
but commit --above <target> -m "message" <id>  # Place the commit above a commit or branch
but commit --below <target> -m "message" <id>  # Place the commit below a commit or branch
but commit -b <branch> --no-message <id>     # Commit without a message
but commit --empty -b <branch> -m "message"  # Insert an empty commit
```

**Where the commit goes:** `-b`/`--branch`, `-A`/`--above`, and `-B`/`--below` are mutually exclusive.

- `-b <branch>` places the commit at the tip of `<branch>`, creating it as an unstacked branch if it does not exist. `-b` with no value creates a branch with a generated name. Targeting a branch that exists but is not applied is an error — except a branch checked out in a linked worktree (experimental worktree flag), which is targeted at its tip, as is a worktree named directly.
- `--above <commit>` / `--below <commit>` insert relative to a commit on that commit's branch. Against a branch, they create a new branch above/below it. Against a linked worktree (experimental worktree flag), `--below` targets the tip of its checked-out branch and `--above` is refused.
- With no branches applied, a new branch is created. With one applied stack, the commit goes to its top branch's tip. With more than one stack, a targeting flag is **required** — otherwise the command fails with "Unclear where to commit. Found more than one stack". The gate is stacks, not branches: several branches stacked together take an untargeted commit on the stack's top branch.

**Important:** `but commit -b <branch> -m "msg"` with no IDs commits ALL uncommitted changes. Pass IDs to commit only specific files or hunks.

`but commit` is not supported from linked worktrees. Use Git directly for the worktree-local commit, and do not run `but setup` there.

**Committing specific files or hunks:** Start with `but diff` for selective dirty commits, then pass CLI IDs as positional arguments:
- **File IDs** from `but diff` or `but status -fv`: commits entire files
- **Hunk IDs** (`<file-id>:<hunk-id>`) from `but diff`: commits individual hunks
- IDs are space-separated (`<id> <id>`). Commas are not separators — `a1,b2` is parsed as a single ID and fails to resolve.

**Placing commits:** Use `--above <target>` or `--below <target>` when the new commit should be inserted at a specific position in existing history. Change-ID refs of existing commits remain valid after an insertion; sha and `#N`-suffixed refs may go stale — add `--status-after` when subsequent history edits need fresh refs.

**Several commits from one diff:** Chain `but commit` calls with `&&` to split a broad uncommitted change into several semantic commits: `but commit -b <branch> -m "msg1" a1 b2 && but commit -b <branch> -m "msg2" c3 d4`. Mutation output is concise by default. Add `--status-after` only when the next step needs workspace IDs or details that the mutation result does not provide. The commits stack in the order you write them — the first `but commit` is the oldest of the new commits and each later one goes on top (newest). File/hunk IDs copied from the original output generally remain usable across commits; if an ID stops resolving, re-read the diff and continue. History edits (`amend`, `squash`, `move`, `uncommit`, `reword`) may run in sequence off one status read when every commit ref involved is a change-ID ref; run them one at a time with `--status-after` when a ref is sha-based or `#N`-suffixed, or when the next command needs freshly issued IDs. Bare `but diff` needs no ID from the preceding command, so `but uncommit <id> && but diff` is safe. If commits from that branch must stay *above* the new ones, see "Split an existing commit" in SKILL.md: commit the replacements, then move the preserved block together with `but move <preserved-id> [<preserved-id>...] -b <branch>` so its internal order stays intact.

Example: `but commit -b my-branch -m "Fix bug" ab cd` commits files/hunks `ab` and `cd`.

Example new branch: `but commit -b feature/contact-form -m "Validate contact form input" ab cd` creates `feature/contact-form` and commits only those selected file or hunk IDs.

To commit specific hunks from a file with multiple changes, use `but diff` to see hunk IDs, then specify them individually.

Edge case: if wanted and unwanted edits are in the same hunk, GitButler cannot split that hunk by ID. Only when the task requires keeping part of that hunk uncommitted, temporarily edit the working tree to isolate the wanted lines, commit those IDs, then restore the leftover lines so they remain uncommitted.

## Editing History

### `but squash <SOURCES>... [-t <target>]`

Move changes into a target. Sources are positional; the target is `-t`/`--target`.
This one command covers squashing, amending, and uncommitting.

```bash
but squash <commit> -t <commit> -m "msg"           # Squash one commit into another
but squash <commit> <commit> -t <commit> -m "msg"  # Squash several commits into a target
but squash <branch> -m "msg"                       # Squash all commits on a branch into one
but squash <branch> -t <commit> -m "msg"           # Squash a branch into a commit, removing the branch
but squash <commit> -t <branch> -m "msg"           # Target a branch: squashes into its newest commit
but squash <file-or-hunk-id> -t <commit>           # Amend an uncommitted change (`but amend` does this)
but squash zz -t <commit>                          # Amend all uncommitted changes into a commit
but squash <commit> -t zz                          # Uncommit a commit
but squash <branch> -t zz                          # Uncommit all commits and remove the branch
but squash <commit-id>:<file-id> -t <commit>       # Move a committed file into another commit
```

All sources must be the same kind (all commits, all branches, all uncommitted changes, `zz`, or all
committed files) and committed-file sources must come from one commit. If `-t` is omitted, `<SOURCES>`
must be exactly one branch, which squashes that branch's commits together.

Message flags (mutually exclusive). Commit and branch sources compose a new message unless the
target is `zz`, so without a flag they open an editor and block — always pass one. Uncommitted and
committed-file sources reuse the target's message and need no flag:

```bash
-m "msg"                # New message; repeat -m to append paragraphs
--no-message            # No message
-u, --use-target-message  # Keep the target's message, drop the sources'
--use-source-message      # Keep the sources' message, drop the target's
```

None of the message flags may be used when the target is `zz`.

For multiple independent squash groups, prefer newer/top groups first; change-ID refs from
one status read stay valid across squashes (the target keeps its ref), so the
groups may run in sequence — use `--status-after` to get fresh refs only
when a ref is sha-based or `#N`-suffixed.

### `but amend -t <commit-or-branch> <SOURCES>...`

Amend uncommitted files/hunks into a specific commit. Use when you know exactly which commit the change belongs to — prefer it over the equivalent `squash` form. Sources must be uncommitted; `amend` rejects commits and committed files, so use `squash` or `move` for those. A branch target resolves to that branch's newest commit, so name the commit explicitly when the change belongs further down.

```bash
but amend -t <commit-id> <file-id> <hunk-id>
but amend -t <branch> <file-id>       # Amends into the branch's newest commit (its tip)
```

Decide the target commit yourself: check `but status -fv`, find the commit the change logically belongs to, then amend into it.

### `but move <SOURCES>... <--above|--below|--branch|--unstack>`

Move commits, committed files, or a branch to a different location. Sources are positional and
space-separated; a target flag is required.

```bash
but move <commit> --below <target-commit>          # Place below target (older) — matches status order
but move <commit> --above <target-commit>          # Place above target (newer)
but move <commit> <commit> --below <target-commit> # Move an adjacent block in one command
but move <commit> <commit> --above <target-commit> # Same block move, anchored from the other side
but move <commit> -b <branch>                      # Move commit to the tip of a branch (created if missing)
but move <commit> --unstack                        # Move commit onto a new unstacked branch
but move <branch> --above <target-branch>          # Stack branch on top of target branch
but move <branch> --unstack                        # Tear off (unstack) a branch
but move <commit-id>:<file-id> --above <commit>    # Move a committed file into a new commit above another
```

Sources may not mix kinds, all committed files must come from the same commit, and only one branch
may be moved at a time. Source order does not matter. For a branch source only `--above` and
`--unstack` apply; `--below` and `-b <name>` require commit or committed-file sources. `--branch`
with no value is equivalent to `--unstack`. With the experimental worktree flag on, `-b` also
accepts a linked worktree or the branch checked out in it, moving commit or committed-file
sources onto that branch's tip (nothing is created); a branch source is refused there.

### `but uncommit <SOURCES>...`

Move commits, branches, or committed files back to the uncommitted area.

```bash
but uncommit <commit-id>                 # Uncommit an entire commit
but uncommit <branch>                    # Uncommit all commits and remove the branch
but uncommit <commit-id>:<file-id>       # Uncommit one file from its commit
```

Multiple whole commits or multiple branches may be passed together, but source kinds cannot be
mixed. Uncommitting a branch also removes an empty branch. Multiple committed-file sources must all
come from the same commit; uncommit files from different commits in separate commands.

When you need file and hunk IDs to recommit selectively, use
`but uncommit <id> && but diff` in one shell call.

### `but reword <id>`

Reword commit message or rename branch.

```bash
but reword <id> -m "new"          # Always pass -m; without it an editor opens and blocks
but reword <branch> -m "new-name" # Rename a branch (applied branches only)
but reword <id> --fix-formatting  # Format to 72-char wrapping
```

### `but discard <CHANGES>...`

Permanently drop branches, commits, or changes. Undo with `but undo`.

```bash
but discard <file-id>              # Discard an uncommitted file's changes
but discard <hunk-id>              # Discard a single hunk
but discard zz                     # Discard all uncommitted changes
but discard <commit-id>            # Drop a commit
but discard <commit-id>:<file-id>  # Drop one file's changes from its commit
but discard <branch>               # Drop a branch and its commits
```

All provided IDs must be the same kind, and committed files must come from the same commit.

## Conflict Resolution

When commits have conflicts (history-editing commands warn about newly conflicted commits in their output; the `but pull` summary lists them; `but status` marks them as conflicted):

### `but resolve <commit>`

Enter resolution mode for a conflicted commit.

```bash
but resolve <commit-id>
```

### `but resolve <path>...`

Mark uncommitted files that `but status` lists as `{conflicted}` resolved with their current worktree content (or as deleted). They then show as ordinary uncommitted changes.

```bash
but resolve src/lib.rs
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
but resolve finish --status-after  # When clearing the last conflict and its workspace is needed
```

The concise result reports leftover markers, surviving uncommitted changes, every remaining
conflicted commit, and the exact current `but resolve <id>` command. Add `--status-after` to the
finish you expect to clear the last conflict only when the task needs the complete resulting
workspace. When it says no conflicted commits remain, stop; do not run a verification status.

### `but resolve cancel`

Cancel conflict resolution and return to workspace mode.

```bash
but resolve cancel
but resolve cancel --force
```

**Workflow:**

1. `but resolve <commit-id>` — enter resolution mode using the commit ID from the `but pull` summary (or `but status`); the conflict regions are printed with line numbers
2. Edit the conflicted files — remove every marker (`<<<<<<<`, `|||||||`, `=======`, `>>>>>>>`; conflicts are diff3-style, so the `|||||||` common-ancestor section is present too) and keep the correct content (`but resolve status` re-lists what remains when several files are conflicted)
3. Finalize with `but resolve finish`; add `--status-after` to the finish you expect to clear the last conflict only when the task needs the complete resulting workspace. When it says no conflicted commits remain, stop; do not run a verification status
4. If multiple commits are conflicted, repeat steps 1-3 for each one, oldest commit first — finishing a lower commit rebases the ones above it

**Important:** Never use `git add`, `git commit`, or other git write commands during conflict resolution. Only use `but resolve` commands and edit files directly.

## Remote Operations

### `but push <branch>`

Push a selected branch and its ancestors to the remote. To update a whole stack, select its top branch once; never loop over the branches. Always specify which branch to push: without one, `but push` prompts for a selection in interactive terminals (one entry per stack, folding in stack ancestors) and otherwise pushes all unpushed work — one push per stack via its topmost unpushed branch, so output has one entry per stack, not per branch. A batch push exits non-zero if any stack failed; stacks that already pushed stay pushed, and rerunning after fixing the failure is safe since up-to-date stacks are skipped. Accepts a full branch name or a branch CLI ID — prefer the name; it stays valid across mutations.

```bash
but push <branch-name>             # Push the selected branch and its ancestors
but push <branch-name> --dry-run   # Preview what would be pushed
but push <branch-name> -s          # Skip force push protection checks
but push <branch-name> --no-hooks  # Bypass pre-push hooks (--no-verify also works)
```

Force push is enabled by default with protection checks. Use `-s` only when intentionally skipping those checks.
After a successful push, GitButler also synchronizes PR targets and stack descriptions for the
selected branch and its ancestors. A forge update failure is reported as a warning; it does not
turn the completed Git push into a failure.

### `but pull`

Update applied branches onto the latest target branch changes (usually `main`).
Use this for "get latest from main" in a GitButler workspace.

```bash
but pull                      # Fetch and rebase applied branches
but pull --check              # Dry-run preview: report what would happen, change nothing
```

Run `but pull` directly for a straightforward update; its output reports the result and `but undo`
reverts it. Use `--check` first when the user or repository policy requires a preview without
updating.
Do not use raw `git pull` or `git rebase`.

### `but pr`

Create and manage pull requests.

```bash
but pr new <branch-selector> -m "Title..."        # Push branch and create PR (recommended); first message line is title, rest is description
but pr new <branch-selector> -F pr_message.txt    # Use file: first line is title, rest is description
but pr new <branch-selector> -t     # Use default content (commit message), skip prompts
but pr new <branch-selector> --draft  # Create as draft
but pr new <branch-selector> --no-hooks  # Bypass pre-push hooks (--no-verify also works)
but pr new <branch-selector> -s     # Skip force-push protection checks
but pr --draft                # Top-level draft flag
but pr auto-merge <selector>  # Enable auto-merge
but pr set-draft <selector>   # Mark review as draft
but pr set-ready <selector>   # Mark review as ready
```

**Key behavior:** `but pr new` automatically pushes the selected branch and its ancestors before creating the PR. No need to run `but push` first. Force push and pre-push hooks run by default.
Use `--no-hooks` to bypass pre-push hooks when needed.
Review creation remains successful if the follow-up stack synchronization fails, and reports that
partial success as a warning.

Selectors for `auto-merge`, `set-draft`, and `set-ready` can be branch names, branch IDs, stack IDs, or numeric review IDs, comma-separated.

Agents must use `--message (-m)`, `--file (-F)`, or `--default (-t)` to avoid editor prompts. The `-t` flag uses the commit message as title/description for single-commit branches; for multi-commit branches it falls back to the branch name as the title.

**Stacked branches:** Use `but pr` for stacked PRs. It creates reviews against the right bases and updates GitButler stack footers in PR descriptions. Creating stacked PRs with `gh pr create` or another forge tool loses that stack-aware behavior. To publish a whole stack, run `but pr new <top-branch-name> -t`; custom messages (`-m` or `-F`) only apply to the selected branch, while dependent branches use default messages (commit title/description).

When the selected branch sits on dependencies that already have PRs, the summary lists those as "PR already exists for ..." and ends with the newly created review. The already-exists lines are normal stack reporting, not a failure to create the selected branch's PR.

Requires forge integration to be configured via `but config forge auth`.

Same-repository pull requests are automatically registered with GitHub's native stacked pull
requests API when the repository is enrolled in GitHub's private preview; otherwise GitButler uses
description footers. `but config forge github-stacks disable` opts out. The setting is
project-local and shared with Desktop.

### `but land <branch>`

Land a branch directly onto the target (e.g. `origin/master`), skipping a pull request. Fast-forwards
when possible, otherwise makes a signed merge commit; for a `gb-local` target it moves the refs
locally. Then reconciles the remaining branches like `but pull`, and deletes each landed branch's
copy on the push remote (only when fully contained in the landed target), reported as
`Deleted <remote>/<branch> (landed)`.

```bash
but land <branch-selector> --yes                  # Land onto the target (--yes required non-interactively)
but land <branch-selector> --no-ff --yes          # Force a merge commit instead of fast-forwarding
but land <top-branch> --whole-stack --yes   # Land an entire stack by naming its top segment
```

Direct target updates are hard to reverse, so confirmation is required (agents must pass `--yes`).
A branch stacked on other segments is refused (its tip would also publish them); `--whole-stack`
is the explicit opt-in, and only the stack's top segment can be named with it.

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

### `but worktree`

Manage linked git worktrees (experimental worktree flag). `but wt` is a default alias.

```bash
but worktree list                 # Active worktrees with IDs, plus the 3 most recent archived ones
but worktree list --archived      # All archived worktrees (`--active` for all active ones)
but worktree archive <id|name>    # Hide a worktree from the workspace
but worktree unarchive <name>     # Show it again; archived worktrees have no ID
but worktree remove [-f] <id|name> # Like `git worktree remove`; `-f` for uncommitted changes
```

Worktrees are listed most recently updated first, as `id name (refs/heads/branch) - path`, with the branch shown only when it differs from the worktree name. Archiving is a GitButler-only state; none of these take part in `but undo`.

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

Converts a regular Git repository to the managed GitButler workspace model. Use `--init` in
non-interactive environments (CI/CD) to ensure a Git repository exists before setup. When the
experimental single-branch feature is enabled, normal CLI use does not require this command: the
repository is registered and its target is inferred lazily without checking out
`gitbutler/workspace` or installing setup hooks.

Rerunning `but setup` on an already-configured project also repairs a missing default target — for
example if `virtual_branches.toml` was reset while the target survived in Git config — so it is the
recovery path when target configuration looks broken.

### `but teardown`

Exit GitButler mode and return to normal git workflow.

```bash
but teardown
```

### `but config`

View and manage GitButler configuration.

```bash
but config
but config user               # Also: forge, target, metrics, feature, ui, ai
but config ai openai          # Also: anthropic, ollama, lmstudio, openrouter
but config target             # Show the current target branch
but config target origin/main # Set the fetch target
but config push-remote         # Show the current push remote
but config push-remote origin  # Set the push remote without changing the target
but config feature                         # List configurable feature flags
but config feature single-branch           # Show a feature flag's value
but config feature single-branch enable    # Also: disable
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

Prefer this reference over exploratory help calls. Use command-specific help when required syntax
is missing or a command fails; use top-level help only to discover an undocumented command.

Full documentation: <https://docs.gitbutler.com/cli-overview>
