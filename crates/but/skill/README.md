# GitButler CLI Skill

Claude Code skill for working with the GitButler CLI (`but` command) in workspace mode.

## Installation

Install this skill using the GitButler CLI:

```bash
but skill install    # Prompts for scope and format
```

The command prompts you to choose local (repository) or global (home directory) scope first, then prompts you to select a skill format (Claude Code, OpenCode, Codex, GitHub Copilot, Cursor, or Windsurf) and installs to the appropriate location. When run outside a git repository, local scope is unavailable and the default install location is global (home directory), but you can still install to a custom location with `--path` using an absolute or `~` path.

**Options:**
- `--path <path>` - Install to a custom path (outside a repository, relative paths require `--global`)
- `--global` - Install globally (in home directory) instead of current repository
- `--detect` - Auto-detect installation location from existing installation (useful for updates)

In non-interactive mode, use `--path` or `--detect`.

**Requirements:**
- **GitButler CLI** installed: `curl -sSL https://gitbutler.com/install.sh | sh`
- **Claude Code, OpenCode, Codex, GitHub Copilot, Cursor, or Windsurf**
- Repository initialized with GitButler: `but setup` (only for local installs)

**Updating:**

To update the skill to the latest version, use the `--detect` flag to automatically detect and update your existing installation:

```bash
but skill install --detect
```

Alternatively, re-run the install command and select the same location:

```bash
but skill install
```

This will overwrite the existing skill files with the latest version.

## Skill Structure

The skill directory contains both distributable skill files and development documentation:

```text
crates/but/skill/
├── SKILL.md                   ← Main skill entry point (INSTALLED)
├── README.md                  ← This file - development docs (NOT installed)
├── RESEARCH.md                ← Tier 4 testing research and strategy (NOT installed)
├── eval/                      ← Tier 4 integration eval harness (NOT installed)
└── references/                ← Additional skill documentation (INSTALLED)
    ├── reference.md           - Command reference
    ├── concepts.md            - Deep concepts
    └── examples.md            - Workflow examples
```

**What gets installed:**
The `but skill install` command only copies the distributable files to the user's system:
- `SKILL.md` - Main skill entry point
- `references/` - All reference documentation files

**What stays in the repository:**
Development documentation remains in the source tree and is not installed:
- `README.md` - This file (development and maintenance docs)
- `RESEARCH.md` - Testing strategy and learnings
- `eval/` - Tier 4 integration test harness and scenarios

## When This Skill Is Invoked

Claude automatically invokes this skill when:

- Checking version control state (status, diffs, commits)
- Starting new work (should create branch/stack for each task)
- After making code changes (should stage files to branches)
- Committing work (when logical units complete)
- Editing history (amend, squash, move changes)
- Any git-like operation

## Progressive Disclosure

Claude loads files on-demand:

1. **SKILL.md** - Always loaded when skill activates (lean overview)
2. **references/reference.md** - Loaded when detailed command syntax needed
3. **references/concepts.md** - Loaded when deeper understanding required
4. **references/examples.md** - Loaded when workflow examples needed

Files in `references/` directory are only loaded when explicitly referenced, keeping context lean while providing comprehensive documentation when needed.

## Key Design Principles

### Trigger-Rich Description

The YAML `description` field contains all triggering information so Claude knows when to use this skill before loading the body.

### Lean Entry Point

SKILL.md is kept under 150 lines as a "table of contents" that points to detailed materials.

### Domain Separation

Separate files by domain (commands, concepts, examples) so Claude only loads relevant context.

### Active Language

Uses directive language ("do this") rather than passive ("this might happen").

## Maintaining This Skill

### When to Update SKILL.md

- New high-level workflow patterns
- Changes to core concepts
- Updates to quick reference commands

### When to Update REFERENCE.md

- New `but` commands
- Changed command syntax
- New flags or options

### When to Update CONCEPTS.md

- New conceptual models
- Changes to workspace behavior
- New architectural patterns

### When to Update EXAMPLES.md

- New workflow patterns
- Common user questions
- Real-world scenarios

### Line Count Guideline

Keep SKILL.md at or under 250 lines. Split content into reference files if approaching the limit.

## Testing the Skill

Test that Claude:

1. Invokes skill when starting new work
2. Creates branches before making changes
3. Stages changes after edits
4. Commits at logical points
5. Uses `but` commands instead of `git`

For automated integration testing against real disposable repositories, use the Tier 4 harness in `eval/`:
Node must satisfy the `promptfoo` engine range documented in `eval/README.md` (repo baseline: `lts/jod`).

```bash
cd crates/but/skill/eval
pnpm install --ignore-workspace
pnpm approve-builds --ignore-workspace
pnpm run eval
# Optional: run the same harness with Codex instead of Claude
pnpm run eval:codex
```

## References

- [Agent Skills Best Practices](https://docs.claude.com/en/docs/agents-and-tools/agent-skills/best-practices)
- [Claude Code Skills Documentation](https://code.claude.com/docs/en/skills)
- [GitButler CLI Documentation](https://docs.gitbutler.com/cli-overview)
