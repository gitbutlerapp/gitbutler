---
name: but
version: 0.0.0
description: Manage GitButler CLI workspace with multiple parallel branches. Use when checking version control state, starting new work (create branch first), committing work, editing history, or any git operation. Uses 'but' commands not 'git' for writes.
author: GitButler Team
---

# GitButler CLI Skill

Help users work with GitButler CLI (`but` command) in workspace mode.

## Proactive Agent Workflow

**CRITICAL:** Follow this pattern for EVERY task involving code changes:

1. **Check state** → `but status --json` (always use `--json` for structured output)
2. **Start work** → `but branch new <task-name>` (create stack for this work theme)
3. **Make changes** → Edit files as needed
4. **Commit work** → `but commit <branch> -m "message" --files <id>,<id>` (commit specific files by CLI ID)
5. **Refine** → Use `but absorb` or `but squash` to clean up history

## After Using Write/Edit Tools

When ready to commit:

1. Run `but status --json` to see uncommitted changes and get their CLI IDs
2. Commit the relevant files directly: `but commit <branch> -m "message" --files <id>,<id>`

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
- When installing or updating this skill programmatically, always use `--path` to specify the exact installation directory. The `--infer` flag requires user interaction if multiple installations exist.
- **Use `--json` flag for all commands** to get structured, parseable output. This is especially important for `but status --json` to reliably parse workspace state.

**Core workflow:**

```bash
but status --json       # Always start here - shows workspace state (JSON for agents)
but branch new feature  # Create new stack for work
# Make changes...
but commit <branch> -m "…" --files <id>,<id>  # Commit specific files by CLI ID
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

**Organizing work:**

- `but branch new <name>` - Independent branch
- `but branch new <name> -a <anchor>` - Stacked branch (dependent)
- `but stage <file> <branch>` - Pre-assign file to branch (optional, for organizing before commit)

**Making changes:**

- `but commit <branch> -m "msg" --files <id>,<id>` - Commit specific files by CLI ID (recommended)
- `but commit <branch> -m "msg"` - Commit ALL uncommitted changes to branch
- `but commit <branch> --only -m "msg"` - Commit only pre-staged changes
- `but amend <file-id> <commit-id>` - Amend file into specific commit (explicit control)
- `but absorb <file-id>` - Absorb file into auto-detected commit (smart matching)
- `but absorb <branch-id>` - Absorb all changes staged to a branch
- `but absorb` - Absorb ALL uncommitted changes (use with caution)

**Editing history:**

- `but rub <source> <dest>` - Universal edit (stage/amend/squash/move)
- `but squash <commits>` - Combine commits
- `but reword <id>` - Change commit message/branch name

**Remote operations:**

- `but pull` - Update with upstream
- `but push [branch]` - Push to remote
- `but pr new <branch>` - Push and create pull request (auto-pushes, no need to push first)

## Key Concepts

For deeper understanding of the workspace model, dependency tracking, and philosophy, see [references/concepts.md](references/concepts.md).

**CLI IDs**: Every object gets a short ID (e.g., `c5` for commit, `bu` for branch). Use these as arguments.

**Direct commit with file IDs**: Use `--files` to commit specific files directly without staging: `but commit <branch> -m "msg" --files <id>,<id>`

**Parallel vs Stacked branches**:

- Parallel: Independent work that doesn't depend on each other
- Stacked: Dependent work where one feature builds on another

**The `but rub` primitive**: Core operation that does different things based on what you combine:

- File + Branch → Stage
- File + Commit → Amend
- Commit + Commit → Squash
- Commit + Branch → Move

## Workflow Examples

For complete step-by-step workflows and real-world scenarios, see [references/examples.md](references/examples.md).

**Starting independent work:**

```bash
but status --json
but branch new api-endpoint
but branch new ui-update
# Make changes, then commit specific files to appropriate branches
but status --json  # Get file CLI IDs
but commit api-endpoint -m "Add endpoint" --files <api-file-id>
but commit ui-update -m "Update UI" --files <ui-file-id>
```

**Cleaning up commits:**

```bash
but absorb              # Auto-amend changes
but status --json       # Verify absorb result
but squash <branch>     # Squash all commits in branch
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
3. Use `--files` to commit specific files directly - no need to stage first
4. Commit at logical units of work
5. **Use `--json` flag for ALL commands** when running as an agent - this provides structured, parseable output instead of human-readable text
6. Use `--dry-run` flags (push, absorb) when unsure
7. Run `but pull` regularly to stay updated with upstream
8. When updating this skill, use `but skill install --path <known-path>` to avoid prompts
