---
name: but
version: 0.0.0
description: Manage GitButler CLI workspace with multiple parallel branches. ALWAYS invoke immediately after Write/Edit tools to stage changes. Use when checking version control state, starting new work (create branch/stack first), after ANY file modifications (stage to branches), committing work, editing history, or any git operation. Uses 'but' commands not 'git' for writes.
author: GitButler Team
---

# GitButler CLI Skill

Help users work with GitButler CLI (`but` command) in workspace mode.

## Proactive Agent Workflow

**CRITICAL:** Follow this pattern for EVERY task involving code changes:

1. **Check state** → `but status --json` (always use `--json` for structured output)
2. **Start work** → `but branch new <task-name>` (create stack for this work theme)
3. **Make changes** → Edit files as needed
4. **Stage changes IMMEDIATELY** → `but stage <file> <branch>` (after EVERY Write/Edit tool use)
5. **Commit work** → `but commit <branch> --only -m "message"` (commit only staged changes)
6. **Refine** → Use `but absorb` or `but squash` to clean up history

**Step 4 is required immediately after any file modification** - do not skip or delay staging.

## After Using Write/Edit Tools

**ALWAYS do this immediately:**

1. Run `but status --json` to see the new changes (use `--json` for structured output)
2. Stage the modified files: `but stage <file-id> <branch-id>`
3. Verify with `but status --json` again

Do not proceed to other tasks until changes are staged.

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
but stage <file> <id>   # Stage changes to branch
but commit <id> --only -m "…"  # Commit only staged changes
but push <id>           # Push to remote
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
- `but stage <file> <branch>` - Assign file to branch

**Making changes:**

- `but commit <branch> --only -m "msg"` - Commit only staged changes
- `but commit <branch> -m "msg"` - Commit ALL uncommitted changes to branch
- `but absorb` - Auto-amend into existing commits

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

**Multiple staging areas**: Each stack has its own staging area. Stage files with `but stage <file> <branch>`.

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
# Make changes, then stage to appropriate branches
but stage <api-file> <api-branch>
but status --json  # Verify staging
but commit <api-branch> --only -m "Add endpoint"
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
3. Stage changes after making edits (especially after formatters/linters)
4. Commit at logical units of work
5. **Use `--json` flag for ALL commands** when running as an agent - this provides structured, parseable output instead of human-readable text
6. Use `--dry-run` flags (push, absorb) when unsure
7. Run `but pull` regularly to stay updated with upstream
8. When updating this skill, use `but skill install --path <known-path>` to avoid prompts
