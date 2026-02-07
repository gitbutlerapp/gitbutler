---
name: but
version: 0.0.0
description: "Commit, push, branch, and manage version control with GitButler. Use for: commit my changes, check what changed, create a PR, push my branch, view diff, create branches, stage files, edit commit history, squash commits, amend commits, undo commits, pull requests, merge, stash work. Replaces git - use 'but' instead of git commit, git status, git push, git checkout, git add, git diff, git branch, git rebase, git stash, git merge. Covers all git, version control, and source control operations."
author: GitButler Team
---

# GitButler CLI Skill

Help users work with GitButler CLI (`but` command) in workspace mode.

## Proactive Agent Workflow

**CRITICAL:** Follow this pattern for EVERY task involving code changes:

1. **Check state** → `but status --json` (always use `--json` for structured output)
2. **Start work** → `but branch new <task-name>` (create stack for this work theme)
3. **Make changes** → Edit files as needed
4. **Commit work** → `but commit <branch> -m "message" --changes <id>,<id> --json --status-after` (commit specific files by CLI ID, get updated status in one call)
5. **Refine** → Use `but absorb <file-id> --json --status-after` or `but squash <branch> --json --status-after` to clean up history

**The Agent Idiom: `--json --status-after`**
Combine these flags on every mutation command. You get structured output AND updated workspace state in a single call:
```bash
but commit <branch> -m "msg" --changes <id>,<id> --json --status-after
# Returns: {"result": {...}, "status": {<full workspace status>}}
# On status failure: {"result": {...}, "status_error": "..."} — handle gracefully
```
This eliminates the need for a separate `but status --json` call after mutations. Use on: `commit`, `absorb`, `rub`, `stage`, `amend`, `squash`, `move`, `uncommit`.

**Commit early, commit often.** Don't hesitate to create commits - GitButler makes editing history trivial. You can always `squash`, `reword`, or `absorb` changes into existing commits later. Small atomic commits are better than large uncommitted changes.

## After Using Write/Edit Tools

When ready to commit:

1. Run `but status --json` to see uncommitted changes and get their CLI IDs
2. Commit the relevant files directly: `but commit <branch> -m "message" --changes <id>,<id> --json --status-after`

You can batch multiple file edits before committing - no need to commit after every single change.

## Critical Concept: Workspace Model

**GitButler ≠ Traditional Git**

- **Traditional Git**: One branch at a time, switch with `git checkout`
- **GitButler**: Multiple stacks simultaneously in one workspace, changes assigned to stacks

**This means:**

- ❌ Don't use `git status`, `git commit`, `git checkout`, `git add`, `git rebase`
- ✅ Use `but status`, `but commit`, `but` commands for all write operations
- ✅ Prefer `but diff` over `git diff` (supports CLI IDs, `--json`, hunk-level output)
- ✅ Read-only git commands are fine for inspection (`git log`, `git blame`)

**Quick translation (git → but):**

| Instead of | Use |
|---|---|
| `git status` | `but status --json` |
| `git add <file>` | `but stage <file> <branch>` or `--changes` on commit |
| `git commit -m "msg"` | `but commit <branch> -m "msg" --json --status-after` |
| `git checkout -b <name>` | `but branch new <name>` |
| `git push` | `but push` or `but pr new <branch>` |
| `git rebase -i` | `but squash`, `but reword`, `but move` |
| `git stash` | `but unapply <branch>` |
| `git cherry-pick` | `but pick <commit> <branch>` |

## Quick Start

**Installation:** `curl -sSL https://gitbutler.com/install.sh | sh && but setup`
**Skill updates:** `but skill check --update` or `but skill install --path <path>`

**Core workflow:**

```bash
but status --json       # Always start here - shows workspace state
but branch new feature  # Create new stack for work
# Make changes...
but commit <branch> -m "…" --changes <id>,<id> --json --status-after  # Commit + get updated status
but push <branch>       # Push to remote
```

## Essential Commands

For detailed command syntax and all available options, see [references/reference.md](references/reference.md).

**IMPORTANT for AI agents:** Add `--json` flag to all commands for structured, parseable output.

**Understanding state:**

- `but status --json` - Overview (START HERE, always use --json for agents)
- `but status --json -f` - Overview with full file lists (use when you need to see all changed files)
- `but show <id> --json` - Details about commit/branch
- `but diff <id>` - Show diff

**Flags explanation:**
- `--json` - Output structured JSON instead of human-readable text (always use for agents)
- `-f` - Include detailed file lists in status output (combines with --json: `but status --json -f`)
- `--status-after` - After a mutation command (`commit`, `absorb`, `rub`, `stage`, `amend`, `squash`, `move`, `uncommit`), also output workspace status. With `--json`, wraps as `{"result": ..., "status": ...}` on success, or `{"result": ..., "status_error": "..."}` if the status query fails. Saves a separate `but status` call.

**JSON output shape (`but status --json`):**
```jsonc
{
  "unassignedChanges": [
    {"cliId": "g0", "filePath": "src/main.rs", "changeType": "modified"}
  ],
  "stacks": [{
    "cliId": "m0",
    "assignedChanges": [{"cliId": "h0", "filePath": "lib.rs", "changeType": "modified"}],
    "branches": [{
      "cliId": "fe", "name": "feature-x",
      "commits": [{
        "cliId": "1b", "commitId": "abc123...", "createdAt": "2025-01-15T10:30:00+00:00",
        "message": "Add feature", "authorName": "Jane Dev", "authorEmail": "jane@example.com",
        "conflicted": null, "reviewId": null, "changes": null
      }],
      "upstreamCommits": [],
      "branchStatus": "unpushedCommits",
      "reviewId": null, "ci": null
    }]
  }],
  "mergeBase": {"cliId": "", "commitId": "def456...", "createdAt": "...", "message": "..."},
  // upstreamState.latestCommit has the same Commit shape as above
  "upstreamState": {"behind": 0, "latestCommit": {"cliId": "...", "commitId": "..."}, "lastFetched": "2025-01-15T10:00:00Z"}
}
```
All Commit objects share the same shape: `cliId`, `commitId`, `createdAt`, `message`, `authorName`, `authorEmail`, `conflicted`, `reviewId`, `changes`. `conflicted` is `null` for upstream commits, `true`/`false` for local. `changes` is `null` unless `-f` is passed. `reviewId` and `ci` are `null` when no forge data is available (use `--refresh-prs` to force a sync).
Use `cliId` values as arguments to other commands (e.g., `--changes g0,h0`). With `--status-after`, mutations return `{"result": <command output>, "status": <above shape>}` (or `"status_error"` instead of `"status"` if the status query fails). Note: IDs are generated per-session — always read them from `but status --json`, don't hardcode them.

**Organizing work:**

- `but branch new <name>` - Independent branch
- `but branch new <name> -a <anchor>` - Stacked branch (dependent)
- `but stage <file> <branch>` - Pre-assign file to branch (optional, for organizing before commit)

**Making changes:**

- `but commit <branch> -m "msg" --changes <id>,<id>` - Commit specific files or hunks (recommended)
- `but commit <branch> -m "msg" -p <id>,<id>` - Same as above, using short flag
- `but commit <branch> -m "msg"` - Commit ALL uncommitted changes to branch
- `but commit <branch> --only -m "msg"` - Commit only pre-staged changes (cannot combine with --changes)
- `but commit <branch> --message-file msg.txt` - Read commit message from file
- `but commit <branch> -c -m "msg"` - Create new branch (or use existing) and commit
- `but amend <file-id> <commit-id>` - Amend file into specific commit (explicit control)
- `but absorb <file-id>` - Absorb file into auto-detected commit (smart matching)
- `but absorb <branch-id>` - Absorb all changes staged to a branch
- `but absorb` - Absorb ALL uncommitted changes (use with caution)

**Getting IDs for --changes:**
- **File IDs**: `but status --json` → each entry in `unassignedChanges` / `assignedChanges` has a `cliId` (e.g., `g0`, `h0`)
- **Hunk IDs**: `but diff --json` (uncommitted changes only) → each entry in `changes[]` has an `id` field (e.g., `e8`, `j0`) for fine-grained commits. When a file has multiple hunks, each hunk is a separate entry with its own `id`. Note: for commit/branch diffs, `id` is absent — those diffs are per-file with hunks nested under `diff.hunks`

**Editing history:**

- `but rub <source> <dest>` - Universal edit (stage/amend/squash/move)
- `but squash <commits>` - Combine commits
- `but reword <id>` - Change commit message/branch name

**Remote operations:**

- `but pull` - Update with upstream
- `but push [branch]` - Push to remote
- `but pr new <branch>` - Push and create pull request (auto-pushes, no need to push first)
- `but pr new <branch> -m "Title..."` - Inline PR message (first line is title, rest is description)
- `but pr new <branch> -F pr_message.txt` - PR message from file (first line is title, rest is description)
- For stacked branches, the custom message (`-m` or `-F`) only applies to the selected branch; dependent branches use defaults

## Key Concepts

For deeper understanding of the workspace model, dependency tracking, and philosophy, see [references/concepts.md](references/concepts.md).

**CLI IDs**: Every object gets a short, unique ID (e.g., `1b` for commit, `fe` for branch "feature-x", `g0` for file). Branch IDs are derived from unique substrings of the branch name; other IDs are auto-generated. Always read IDs from `but status --json` — they are generated per-session.

**Parallel vs Stacked branches**:

- Parallel: Independent work that doesn't depend on each other
- Stacked: Dependent work where one feature builds on another

**The `but rub` primitive**: Core operation that does different things based on what you combine:

- File + Branch → Stage
- File + Commit → Amend
- Commit + Commit → Squash
- Commit + Branch → Move
- File/Commit + `zz` → Unstage/Undo (back to unassigned)
- `zz` + Branch/Commit → Stage/Amend all unassigned changes
- File-in-Commit + `zz` → Uncommit specific file

## Workflow Examples

For complete step-by-step workflows and real-world scenarios, see [references/examples.md](references/examples.md).

**Starting independent work:**

```bash
but status --json
but branch new api-endpoint
but branch new ui-update
# Make changes, then commit specific files to appropriate branches
but status --json  # Get file CLI IDs
but commit api-endpoint -m "Add endpoint" --changes <api-file-id> --json --status-after
but commit ui-update -m "Update UI" --changes <ui-file-id> --json --status-after
```

**Committing specific hunks (fine-grained control):**

```bash
but diff --json             # See hunk IDs when a file has multiple changes
but commit <branch> -m "Fix first issue" --changes <hunk-id-1> --json --status-after
but commit <branch> -m "Fix second issue" --changes <hunk-id-2> --json --status-after
```

**Cleaning up commits:**

```bash
but absorb <file-id> --json --status-after   # Auto-amend specific file into its commit
but absorb <branch-id> --json --status-after  # Absorb all changes staged to a branch
but squash <branch> --json --status-after  # Squash all commits in branch + get updated status
```

**Resolving conflicts:**

```bash
but resolve <commit>    # Enter resolution mode
# Fix conflicts in editor
but resolve finish      # Complete resolution
```

## Common Mistakes

- **Using `git` for writes:** Never use `git commit`, `git status`, `git add`, `git checkout`, `git rebase`. Use `but` equivalents. Prefer `but diff` over `git diff`. Read-only git commands like `git log` and `git blame` are fine.
- **Forgetting `--json`:** Always add `--json` for machine-readable output. Without it, you get human-formatted text that's harder to parse reliably.
- **Bare `but absorb`:** Without arguments, absorbs ALL uncommitted changes across all branches. Prefer targeted absorb: `but absorb <file-id>` or `but absorb <branch-id>`. Only use bare `but absorb` when you intentionally want to absorb everything.
- **`--only` with `--changes`:** These flags are mutually exclusive. Use `--changes <id>,<id>` to commit specific files (recommended for agents), or `--only` to commit pre-staged changes.
- **Skipping `--status-after`:** On mutation commands, always add `--status-after` to get updated workspace state without a separate round-trip.
- **Committing without `--changes`:** Without `--changes`, `but commit <branch> -m "msg"` commits ALL uncommitted changes to that branch — not just the ones you intended. Always specify `--changes` for precision.

## Guidelines

1. Always start with `but status --json` to understand current state (agents should always use `--json`)
2. Create a new stack for each independent work theme
3. Use `--changes` to commit specific files directly - no need to stage first
4. **Commit early and often** - don't wait for perfection. Unlike traditional git, GitButler makes editing history trivial with `absorb`, `squash`, and `reword`. It's better to have small, atomic commits that you refine later than to accumulate large uncommitted changes.
5. **Use `--json` flag for ALL commands** when running as an agent - this provides structured, parseable output instead of human-readable text
6. **Use `--status-after`** on mutation commands (`commit`, `absorb`, `rub`, `stage`, `amend`, `squash`, `move`, `uncommit`) to get workspace status in the same call — avoids a separate `but status` round-trip
7. Use `--dry-run` flags (push, absorb) when unsure
8. Run `but pull` regularly to stay updated with upstream
9. When updating this skill, use `but skill install --path <known-path>` to avoid prompts
10. Run `but skill check` to verify your skill files are up to date, or `but skill check --update` to auto-update outdated installations
