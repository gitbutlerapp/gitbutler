# Tier 4 Integration Eval Harness

This directory implements Tier 4 testing from `../RESEARCH.md`: run a real Claude Code agent against a disposable repository and assert behavior from command traces and final `but status --json`.

## What it tests

- Real `but` binary execution (`target/debug/but`)
- Real Git repository state changes in per-test fixtures
- Skill-guided behavior from installed `.claude/skills/gitbutler`
- Command sequencing and flag compliance (`--json`, `--status-after`, `--changes`)

## Prerequisites

- Claude Code CLI is installed and up to date. For `auto`/`local`, log in (`claude` then `/login`). For `api`, provide `ANTHROPIC_API_KEY` (or `BUT_EVAL_ANTHROPIC_API_KEY`); login is not required.
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

# Open promptfoo report UI
pnpm run view
```

## Auth modes

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
- You can override the minimum with `BUT_EVAL_MIN_CLAUDE_VERSION`.
- The provider enforces a per-test Claude timeout (default `180000` ms).
- You can override timeout via `providers[].config.claude_timeout_ms` or `BUT_EVAL_CLAUDE_TIMEOUT_MS`.
- You can override the Claude executable path with `BUT_EVAL_CLAUDE_BIN`.

## Files

- `promptfooconfig.yaml`: Tier 4 scenarios and assertion wiring
- `assertions/but-assertions.ts`: shared assertion logic loaded directly by promptfoo (`file://...:functionName`)
- `providers/but-integration.ts`: source for the promptfoo custom provider
- `providers/claude-local.sh`: wrapper that runs `claude -p` and supports `auto|local|api` auth modes
- `dist/providers/but-integration.js`: compiled provider used by promptfoo
- `setup-fixture.sh`: creates disposable repositories and installs the skill into each fixture

## Debugging

Set `keep_fixtures: true` under `providers[].config` in `promptfooconfig.yaml` to preserve fixture directories in provider output while debugging.
