# Tier 4 Integration Eval Harness

This directory implements Tier 4 testing from `../RESEARCH.md`: run a real coding agent (Claude Code or Codex) against a disposable repository and assert behavior from command traces and final `but status --json`.

## What it tests

- Real `but` binary execution (`target/debug/but`)
- Real Git repository state changes in per-test fixtures
- Skill-guided behavior from installed skill in fixture (`.claude/skills/gitbutler` and `.codex/skills/gitbutler`)
- Command sequencing and flag compliance (`--json`, `--status-after`, `--changes`)

## Prerequisites

- Claude Code CLI is installed and up to date for Claude runs. For `auto`/`local`, log in (`claude` then `/login`). For `api`, provide `ANTHROPIC_API_KEY` (or `BUT_EVAL_ANTHROPIC_API_KEY`); login is not required.
- Codex CLI is installed and logged in for Codex runs (`codex login`).
- Node.js version compatible with `promptfoo` engines (currently `^20.20.0 || >=22.22.0`; recommended: `lts/jod` from repo `.nvmrc`)
- Rust toolchain installed
- GitButler must be initialized in the test repo (`but setup`) before running commands. The harness enforces this for each fixture.

## Install dependencies

```bash
cd crates/but/skill/eval
pnpm install --ignore-workspace
# One-time on fresh machines: approve native build deps used by promptfoo
pnpm approve-builds --ignore-workspace
```

## Run evals

```bash
# One run of each scenario
pnpm run eval

# Repeat scenarios for variance tracking
pnpm run eval:repeat

# One run of each scenario using Codex
pnpm run eval:codex

# Repeat Codex scenarios for variance tracking
pnpm run eval:codex:repeat

# Open promptfoo report UI
pnpm run view
```

## Auth modes

Auth modes apply to the Claude runner.

The runner supports three auth modes, configured via `providers[].config.auth_mode` in `promptfooconfig.yaml`:

- `auto` (default): uses `ANTHROPIC_API_KEY` when it is set; otherwise uses Claude Code account auth
- `local`: always uses Claude Code account auth
- `api`: always uses API-key auth and requires `ANTHROPIC_API_KEY` (or `BUT_EVAL_ANTHROPIC_API_KEY`)

Examples:

```bash
# Force local/account auth for this run
BUT_EVAL_AUTH_MODE=local pnpm run eval

# Force API-key auth for this run
BUT_EVAL_AUTH_MODE=api ANTHROPIC_API_KEY=... pnpm run eval
```

## Runner safeguards

- The Claude runner validates CLI version and requires `>= 1.0.88` by default.
- When multiple `claude` binaries are present on PATH, the runner auto-selects the newest one that satisfies the minimum version.
- The Codex runner validates CLI version and requires `>= 0.99.0` by default.
- Override minimum version with `BUT_EVAL_MIN_CLAUDE_VERSION`, `BUT_EVAL_MIN_CODEX_VERSION`, or `BUT_EVAL_MIN_RUNNER_VERSION`.
- The provider enforces a per-test runner timeout (default `180000` ms).
- Override timeout via `providers[].config.runner_timeout_ms`, `providers[].config.claude_timeout_ms`, `BUT_EVAL_RUNNER_TIMEOUT_MS`, or `BUT_EVAL_CLAUDE_TIMEOUT_MS`.
- Override executable paths with `BUT_EVAL_CLAUDE_BIN`, `BUT_EVAL_CODEX_BIN`, or `BUT_EVAL_RUNNER_BIN`.

## Files

- `promptfooconfig.yaml`: Tier 4 scenarios and assertion wiring
- `assertions/but-assertions.ts`: shared assertion logic loaded directly by promptfoo (`file://...:functionName`)
- `providers/but-integration.ts`: source for the promptfoo custom provider
- `providers/claude-local.sh`: wrapper that runs `claude -p` and supports `auto|local|api` auth modes
- `providers/codex-local.sh`: wrapper that runs `codex exec --json` in full-access, no-approval mode
- `dist/providers/but-integration.js`: compiled provider used by promptfoo
- `setup-fixture.sh`: creates disposable repositories and installs the skill into each fixture

## Debugging

Set `keep_fixtures: true` under `providers[].config` in `promptfooconfig.yaml` to preserve fixture directories in provider output while debugging.
