# Tier 4 Integration Eval Harness

This directory implements Tier 4 testing from `../RESEARCH.md`: run a real Claude Code agent against a disposable repository and assert behavior from command traces and final `but status --json`.

## What it tests

- Real `but` binary execution (`target/debug/but`)
- Real Git repository state changes in per-test fixtures
- Skill-guided behavior from installed `.claude/skills/gitbutler`
- Command sequencing and flag compliance (`--json`, `--status-after`, `--changes`)

## Prerequisites

- `ANTHROPIC_API_KEY` is set
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

## Files

- `promptfooconfig.yaml`: Tier 4 scenarios and assertion wiring
- `assertions/but-assertions.ts`: shared assertion logic loaded directly by promptfoo (`file://...:functionName`)
- `providers/but-integration.ts`: source for the promptfoo custom provider
- `dist/providers/but-integration.js`: compiled provider used by promptfoo
- `setup-fixture.sh`: creates disposable repositories and installs the skill into each fixture

## Debugging

Set `keep_fixtures: true` under `providers[].config` in `promptfooconfig.yaml` to preserve fixture directories in provider output while debugging.
