# but command reference

- IDs for commits, branches, files, and hunks: `but help cli-ids`.
- Terms (workspace, applied, stack, target, `@`): `but skill concepts`.
- Every command accepts `--json`. Mutations accept `--status-after` to append the resulting workspace status.
- History edits refuse branches and commits already merged upstream; `--allow-merged` overrides that.
- Mutations are recorded in the operation log; `but undo` reverts the last one.
- To run against another directory, put `-C <path>` before the command: `but -C <path> <cmd>`.
- Examples and details: `but <cmd> --help`.

## Inspection

### but status
Show an overview of the workspace state
- `-f` Also list the files changed by each commit
- `-v, --verbose` Show verbose output with commit author and timestamp
- `-u, --upstream` Show detailed list of upstream commits that haven't been integrated yet

### but diff [TARGET]
Show the diff of changes in the repo
- `[TARGET]` What to diff, by CLI ID: a commit, branch, committed file or hunk, uncommitted file or hunk, path prefix, or a worktree's uncommitted area. A commit lists its files and hunks with their IDs. If omitted shows the diff of all uncommitted changes, with file and hunk IDs. For more details about CLI IDs, see but help cli-ids.

### but show <COMMIT_OR_BRANCH>
Show details of a commit or branch
- `<COMMIT_OR_BRANCH>` The commit ID (short or full SHA), branch name, or CLI ID to show details for
- `-v, --verbose` Show full commit messages and files changed for each commit

## Branching and Committing

### but commit [CHANGES]...
Create a commit
- `[CHANGES]...` The files or hunks to commit, by CLI ID from but diff. If omitted, everything uncommitted is committed.
- `-m, --message <MESSAGE>` The message to use for the commit. Can be supplied any amount of times, each value being appended to the preceding ones with a blank line in between. Without -m or --no-message, a terminal opens the editor and a non-interactive run commits with an empty message.
- `--no-message` Create the commit without a message
- `-b, --branch [<BRANCH>]` Place the commit on the branch BRANCH. If BRANCH does not exist, it is created as an unstacked branch. If BRANCH is omitted, an unstacked branch with a generated name is created. If BRANCH is a worktree or a branch checked out in one, the commit is placed on the tip of that worktree's branch. Attempting to place a commit on a branch that exists but is not applied is an error.
- `-A, --above <BRANCH_OR_COMMIT>` Place the commit above BRANCH_OR_COMMIT, which must be an applied branch or commit. If BRANCH_OR_COMMIT is a commit, the new commit is placed on the same branch as the targeted commit. If BRANCH_OR_COMMIT is a branch, the new commit is placed on a new branch above the targeted branch.
- `-B, --below <BRANCH_OR_COMMIT>` Place the commit below BRANCH_OR_COMMIT, which must be an applied branch or commit. If BRANCH_OR_COMMIT is a commit, the new commit is placed on the same branch as the targeted commit. If BRANCH_OR_COMMIT is a branch, the new commit is placed on a new branch below the targeted branch. Branches are treated as buckets, meaning that "below a branch" is treated as below the oldest ancestor on that branch. If BRANCH_OR_COMMIT is a worktree, the new commit is placed on the tip of the branch that worktree has checked out.
- `--empty` Create an empty commit even when there are changes

### but branch new [NAME]
Create a new branch
- `[NAME]` Name of the new branch. If omitted the new branch will get a generated name.
- `-A, --above <BRANCH_OR_COMMIT>` Place the branch above BRANCH_OR_COMMIT, which must be an applied branch or commit. If BRANCH_OR_COMMIT is a commit, the new branch is created above the commit. If BRANCH_OR_COMMIT is a branch, the new branch is created above the targeted branch.
- `-B, --below <BRANCH_OR_COMMIT>` Place the branch below BRANCH_OR_COMMIT, which must be an applied branch or commit. If BRANCH_OR_COMMIT is a commit, the new branch is created below the commit. If BRANCH_OR_COMMIT is a branch, the new branch is created below the targeted branch.

### but branch delete <BRANCHES>...
Delete branches and their commits from the workspace
- `<BRANCHES>...` One or more branches to delete

### but branch list [FILTER]
List the branches in the repository
- `[FILTER]` Filter branches by name (case-insensitive substring match)
- `-l, --local` Show only local branches
- `-r, --remote` Show only remote branches
- `-a, --all` Show all branches (not just active + 20 most recent)

### but branch show <BRANCH>
Show commits ahead of base for a specific branch
- `<BRANCH>` CLI ID or name of the branch to show
- `-f, --files` Show files modified in each commit with line counts
- `--check` Check if the branch merges cleanly into upstream and identify conflicting commits

### but branch update <BRANCH>
Integrate a branch's remote counterpart into the local branch
- `<BRANCH>` Name of the local branch to integrate
- `-s, --strategy <STRATEGY>` Strategy to use for the integration [possible values: pull-rebase, smart-squash, merge, pick-remote] [default: pull-rebase]
- `--dry-run` Preview the resulting branch state without persisting changes
- `-v, --verbose` Show additional dry-run details like the current divergence

### but discard [CHANGES]...
Discard branches, commits, or changes
- `[CHANGES]...` One or more branches, commits, or changes to discard. If omitted all uncommitted changes will be discarded.

### but resolve [TARGETS]...
Resolve conflicts in a commit or in uncommitted files
- `[TARGETS]...` A commit to enter resolution mode for, or one or more conflicted uncommitted files (as listed by but status) to mark as resolved with their current worktree content. Resolution mode checks the commit out until finish or cancel; the conflicts and apply subcommands resolve a commit without it
- `--ai` Resolve the conflicts with the configured AI model and apply the result. With a commit ID this resolves only that commit; without one it resolves all conflicted commits in the workspace, oldest first. Undo the result with but undo.

### but resolve conflicts [COMMIT]
List the conflicts of a conflicted commit, without entering resolution mode
- `[COMMIT]` A conflicted commit, or a branch (meaning its oldest conflicted commit). Defaults to the first conflicted branch's oldest conflicted commit

### but resolve apply <TARGET>
Resolve conflicts of a conflicted commit, without entering resolution mode
- `<TARGET>` The conflicted file, optionally with a 1-based conflict number (<path>:<N>)
- `--commit <COMMIT>` A conflicted commit, or a branch (meaning its oldest conflicted commit — branch names stay stable across applies, unlike commit ids). Defaults to the first conflicted branch's oldest conflicted commit
- `--ours` Take the ours side: the new base the commit was rebased onto
- `--theirs` Take the theirs side: the commit's own version
- `--ai` Let the configured AI model merge the targeted conflicts
- `-F, --file <FILE>` Read the replacement content from this file (otherwise from stdin)

### but resolve status
Show the status of conflict resolution, listing remaining conflicted files

### but resolve finish
Finalize conflict resolution and return to workspace mode

### but resolve cancel
Cancel conflict resolution and return to workspace mode
- `-f, --force` Discard any changes made during resolution

### but unapply <BRANCH_OR_STACK>
Remove a branch from the workspace, keeping it to apply again later
- `<BRANCH_OR_STACK>` The branch or stack to unapply

### but apply <BRANCH>
Apply a branch
- `<BRANCH>` The branch to apply

### but clean
Remove empty branches from the workspace
- `--dry-run` Preview which branches would be removed without actually deleting them
- `--pull` Pull latest changes from the remote before cleaning
- `--include-upstream` Also remove branches that have upstream-only commits but no local commits or changes

### but pick <SOURCES>...
Cherry-pick commits into an applied branch
- `<SOURCES>...` The commits to copy, as SHAs or as CLI IDs of commits on applied branches. IDs shown for unapplied branches do not resolve; use the SHA
- `-b, --branch [<BRANCH>]` Place the picked commits on the branch BRANCH. If BRANCH does not exist, it is created as an unstacked branch. If BRANCH is omitted, an unstacked branch with a generated name is created. Attempting to pick onto a branch that exists but is not applied is an error.
- `-A, --above <BRANCH_OR_COMMIT>` Place the picked commits above BRANCH_OR_COMMIT. If BRANCH_OR_COMMIT is a commit, the picked commits are placed on the same branch as the targeted commit. If BRANCH_OR_COMMIT is a branch, the picked commits are placed on a new branch above the targeted branch.
- `-B, --below <BRANCH_OR_COMMIT>` Place the picked commits below BRANCH_OR_COMMIT. If BRANCH_OR_COMMIT is a commit, the picked commits are placed on the same branch as the targeted commit. If BRANCH_OR_COMMIT is a branch, the picked commits are placed on a new branch below the targeted branch. Branches are treated as buckets, meaning that "below a branch" is treated as below the oldest ancestor on that branch.

## Editing Commits

### but squash [SOURCES]...
Squash commits, branches, or changes
- `[SOURCES]...` The sources to squash, all of one kind. Commits: squashed into the target. Branches: every commit on them is squashed into the target and the branches are removed; with no target and exactly one branch, that branch is squashed into a single commit. Uncommitted files or hunks, or @ for all of them: squashed into the target. Committed files and hunks from one commit: moved into the target. A target of @ uncommits the sources instead. With --target and no sources, @ is used.
- `-m, --message <MESSAGE>` The message to use for the new commit. Can be supplied any number of times, each value being appended to the preceding ones with a blank line in between. Without a message flag, squashing commits or branches opens the editor in a terminal; a non-interactive run skips the editor. This cannot be used when TARGET is the uncommitted area (@).
- `--no-message` Create the commit without a message. This cannot be used when TARGET is the uncommitted area (@).
- `-u, --use-target-message` Use the message of the target. The message of the source(s) will be discarded. This cannot be used when TARGET is the uncommitted area (@).
- `--use-source-message` Use the message of the source(s). The message of the target will be discarded. Cannot be used if <SOURCES> are not committed, if TARGET is the uncommitted area (@), or if moving committed changes between commits.
- `-t, --target <TARGET>` The target to squash into. If TARGET is a commit the sources will be added to the commit. If TARGET is a branch the sources will be added to that branch's newest commit (its tip). If TARGET is the uncommitted area (@) the sources will be uncommitted. A commit owned by a worktree uncommits into that worktree's area instead, named by its ID (<id>:@).

### but move <SOURCES>...
Move commits and changes around
- `<SOURCES>...` One or more sources to move, all of one kind: commits; committed files and hunks from one commit; or a single branch. The order of the sources does not matter. Providing any of the sources as an argument for a target such as --above or --below is an error.
- `-b, --branch [<BRANCH>]` Place <SOURCES> on the branch BRANCH. If BRANCH exists, commits or committed changes are moved onto its tip. A branch source is instead stacked on top of BRANCH, equivalent to --above BRANCH. If BRANCH does not exist, it is created as an unstacked branch for commit or committed-change sources. Using a branch source with a nonexistent BRANCH is an error. If BRANCH is a worktree or a branch checked out in one, commit or committed-change sources are moved onto the tip of that worktree's branch. If BRANCH is omitted, an unstacked branch with a generated name is created. This is exactly equivalent to --unstack and is allowed for any source kind. Attempting to place <SOURCES> on a branch that exists but is not applied is an error.
- `-A, --above <BRANCH_OR_COMMIT>` Place <SOURCES> above BRANCH_OR_COMMIT. If BRANCH_OR_COMMIT is a commit, <SOURCES> are placed on the same branch as the targeted commit. If BRANCH_OR_COMMIT is a branch, the sources are placed on a new branch above the targeted branch. This target is applicable for all kinds of <SOURCES>.
- `-B, --below <BRANCH_OR_COMMIT>` Place <SOURCES> below BRANCH_OR_COMMIT. If BRANCH_OR_COMMIT is a commit, the <SOURCES> are placed on the same branch as the targeted commit. If BRANCH_OR_COMMIT is a branch, <SOURCES> are placed on a new branch below the targeted branch. Branches are treated as buckets, meaning that "below a branch" is treated as below the oldest ancestor on that branch. If BRANCH_OR_COMMIT is a worktree, <SOURCES> are placed on the tip of the branch that worktree has checked out. This target is only applicable for <SOURCES> that are commits or committed changes.
- `--unstack` Unstack <SOURCES> from their current stacks. --unstack does not take an argument, so --unstack <SOURCES> and <SOURCES> --unstack are equivalent.

### but split <SOURCES>...
Split a commit in two
- `<SOURCES>...` The committed files and hunks to move into a new commit

### but absorb [SOURCE]
Amend uncommitted changes into the commits they belong to
- `[SOURCE]` An uncommitted file or hunk to absorb; if omitted, everything uncommitted is absorbed
- `--dry-run` Show the absorption plan without making any changes

### but reword <TARGET>
Edit the commit message of the specified commit
- `<TARGET>` Commit ID to edit, branch ID to rename, or anonymous branch ID to name
- `-m, --message <MESSAGE>` The new commit message or branch name. Without it, a terminal opens the editor and a non-interactive run fails
- `-f, --fix-formatting` Format the existing commit message to 72-char line wrapping without opening an editor

### but uncommit <SOURCES>...
Move commits, branches, or committed changes back into the uncommitted area
- `<SOURCES>...` One or more commits, branches, or committed changes to uncommit. Sources must all be the same category.

### but amend [SOURCES]...
Amend uncommitted changes into a commit or branch
- `[SOURCES]...` One or more uncommitted files or hunks to amend. If omitted, all changes in the uncommitted area (@) are amended.
- `-t, --target <COMMIT_OR_BRANCH>` The commit to amend into; a branch means its newest commit

## Operation History

### but oplog list
List operation history
- `--since <SINCE>` Start from this oplog SHA instead of the head
- `-s, --snapshot` Show only on-demand snapshot entries

### but oplog snapshot
Create an on-demand snapshot with optional message
- `-m, --message <MESSAGE>` Message to include with the snapshot

### but oplog restore <OPLOG_SHA>
Restore to a specific oplog snapshot
- `<OPLOG_SHA>` Oplog SHA to restore to

### but undo
Undo the last operation

### but redo
Redo the last undo

## Server Interactions

### but merge <BRANCH>
Merge a branch directly onto the target branch, bypassing review
- `<BRANCH>` Branch ID or name to merge onto the target branch
- `--yes` Skip the confirmation prompt
- `--no-ff` Always create a merge commit, even when the branch can be fast-forwarded
- `--whole-stack` Merge the entire stack: BRANCH must be the top segment, and the segments below it are published to the target along with it

### but push [BRANCH]
Push changes in a branch to remote
- `[BRANCH]` Branch name or CLI ID to push; the branches below it in its stack are pushed with it. If omitted, a terminal prompts for a selection and a non-interactive run pushes every stack with unpushed commits
- `-f, --with-force` Force push even if it's not fast-forward
- `-s, --skip-force-push-protection` Skip force push protection checks
- `--no-hooks` Bypass pre-push hooks
- `-d, --dry-run` Show what would be pushed without actually pushing

### but pull
Update all applied branches onto the latest target branch
- `-c, --check` Only check whether the update would apply cleanly, without updating

### but pr
Commands for creating and managing reviews on a forge, e.g. GitHub PRs or GitLab MRs
- `-d, --draft` Create the review as a draft

### but pr new [BRANCH]
Create a new review for a branch, force-pushing it first
- `[BRANCH]` The branch to create a review for. Without it, a terminal prompts for one (or confirms when only one branch lacks a review); a non-interactive run needs it
- `-m, --message <MESSAGE>` Review title and description: the first line is the title, the rest is the description. A non-interactive run needs -m, -F, or -t
- `-F, --file <FILE>` Read review title and description from file. The first line is the title, the rest is the description
- `-s, --skip-force-push-protection` Skip force push protection checks
- `--no-hooks` Bypass pre-push hooks
- `-t, --default` Use the default content for the review title and description, skipping any prompts. If the branch contains only a single commit, the commit message will be used
- `-d, --draft` Create the review as a draft

### but pr auto-merge [SELECTOR]
Enable or disable the automatic merging of reviews
- `[SELECTOR]` One or more comma-separated branch names, branch IDs, stack IDs (every review on the stack), or review numbers (the PR or MR number without the symbol). Without it, a terminal prompts for reviews from the workspace's branches; a non-interactive run needs it
- `-d, --off` Disable automatic merging instead of enabling it

### but pr set-draft [SELECTOR]
Mark existing reviews as draft
- `[SELECTOR]` One or more comma-separated branch names, branch IDs, stack IDs (every review on the stack), or review numbers (the PR or MR number without the symbol). Without it, a terminal prompts for reviews from the workspace's branches; a non-interactive run needs it

### but pr set-ready [SELECTOR]
Mark existing reviews as ready for review
- `[SELECTOR]` One or more comma-separated branch names, branch IDs, stack IDs (every review on the stack), or review numbers (the PR or MR number without the symbol). Without it, a terminal prompts for reviews from the workspace's branches; a non-interactive run needs it

### but pr template [TEMPLATE_PATH]
Configure the template to use for review descriptions
- `[TEMPLATE_PATH]` Path to the review template file within the repository. Without it, a terminal lists the templates found in the repository to pick from; a non-interactive run needs it

## Other Commands

### but setup
Set up a GitButler project from the git repository in the current directory
- `--init` Initialize a new git repository with an empty commit if one doesn't exist. Useful in non-interactive environments such as CI, where a repository may not exist yet.

### but update check
Check if a new version of the GitButler CLI is available

### but config forge auth
Authenticate with the forge (GitHub, GitLab, or Bitbucket)
