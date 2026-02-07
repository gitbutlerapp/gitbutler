---
name: but
version: 0.0.0
description: Commit, push, branch, and manage version control. Use for git commit, git status, git push, git diff, creating branches, staging files, editing history, pull requests, or any git/version control operation. Replaces git write commands with 'but' - always use this instead of raw git.
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
5. **Refine** → Use `but absorb --json --status-after` or `but squash --json --status-after` to clean up history

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

- ❌ Don't use `git status`, `git commit`, `git checkout`
- ✅ Use `but status`, `but commit`, `but` commands
- ✅ Read-only git commands are fine (`git log`, `git diff`)

## Quick Start

**Installation:**

```bash
curl -sSL https://gitbutler.com/install.sh | sh
but setup                          # Initialize in your repo
but skill install --path <path>    # Install/update skill (agents use --path with known location)
```

**Note for AI agents:**
- When installing or updating this skill programmatically, always use `--path` to specify the exact installation directory. The `--detect` flag requires user interaction if multiple installations exist.
- **Use `--json` flag for all commands** to get structured, parseable output. This is especially important for `but status --json` to reliably parse workspace state.

**Core workflow:**

```bash
but status --json       # Always start here - shows workspace state (JSON for agents)
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
- `--status-after` - After a mutation command (`commit`, `absorb`, `rub`, `stage`, `amend`, `squash`, `move`, `uncommit`), also output workspace status. With `--json`, wraps both in `{"result": ..., "status": ...}`. Saves a separate `but status` call.

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
- **File IDs**: `but status --json` - commit entire files
- **Hunk IDs**: `but diff --json` - commit individual hunks (for fine-grained control when a file has multiple changes)

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

**CLI IDs**: Every object gets a short ID (e.g., `c5` for commit, `bu` for branch). Use these as arguments.

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
but absorb --json --status-after    # Auto-amend changes + get updated status in one call
but squash <branch> --json --status-after  # Squash all commits in branch + get updated status
```

**Resolving conflicts:**

```bash
but resolve <commit>    # Enter resolution mode
# Fix conflicts in editor
but resolve finish      # Complete resolution
```

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
