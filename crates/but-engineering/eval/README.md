# Tier 4 Coordination Eval Harness

This directory contains Tier-4 evaluations for `but-engineering` using real coding agents (Claude by default, Codex optionally) in disposable git repositories.

Each fixture is preflight-validated as a GitButler repo (`but setup` + `but status --json`) before scenarios run, so core-flow results are not polluted by missing setup.

## Goals

The deterministic default gate validates coordination behaviors:

1. Conflict blocking on claimed files
2. Advisory plus explicit `check` usage before allowed edits
3. Plan/discover discipline with claim cleanup
4. Stack dependency coordination via `but status --json` and `check --include-stack`
5. Commit-lock recovery by coordinating and aligning a stacked branch

A separate natural-behavior suite validates emergent coordination without
prompting exact command sequences.

A separate composite track validates a demo-style end-to-end flow where one
agent ships independent work, discovers a gotcha, and coordinates around stack
dependencies without overwriting a contested base file.

## Prerequisites

- Claude Code CLI installed and authenticated for default runs
- Node.js compatible with `promptfoo` (`^20.20.0 || >=22.22.0`)
- Rust toolchain

Optional:

- Codex CLI authenticated for `eval:codex`

## Install

```bash
cd crates/but-engineering/eval
pnpm install --ignore-workspace
pnpm approve-builds --ignore-workspace
```

## Run

```bash
# Claude-first default gate
pnpm run eval

# Stability sampling
pnpm run eval:repeat

# Natural-behavior suite
pnpm run eval:natural

# Natural-behavior repeat run + threshold metrics
pnpm run eval:natural:metrics

# Composite demo track (non-gating while tuning)
pnpm run eval:composite
pnpm run eval:composite:metrics
pnpm run eval:composite:diagnose

# Natural-behavior failure diagnostics (per failed row)
pnpm run eval:natural:failures

# One-shot tuning loop: repeat + metrics + failure diagnostics
pnpm run eval:natural:diagnose

# Optional compatibility baseline
pnpm run eval:codex

# Optional Codex natural-behavior baseline
pnpm run eval:natural:codex
```

## Harness Layout

- `promptfooconfig.yaml`: scenarios and assertion wiring
- `promptfooconfig.natural.yaml`: natural-behavior scenarios (no exact command hints)
- `promptfooconfig.composite.yaml`: single composite demo scenario
- `providers/engineering-integration.ts`: custom promptfoo provider
- `providers/claude-local.sh`: Claude runner wrapper
- `providers/codex-local.sh`: Codex runner wrapper (optional runs)
- `assertions/engineering-assertions.ts`: behavioral pass/fail checks
- `scripts/natural-metrics.ts`: repeat-run KPI/threshold aggregation
- `scripts/natural-failures.ts`: failed-row signal diagnostics for tuning
- `setup-fixture.sh`: disposable repo bootstrap with skill + hook setup

## Output Contract

Provider output is a JSON string containing:

- `taskPrompt`: effective prompt sent to the runner
- `commands`: extracted command trace (`command`, `failed`)
- `editOperations`: extracted `Edit`/`Write`/`MultiEdit` tool operations with event indexes
- `coordinationState`: post-run `agents`, `claims`, `messages`, `discoveries`, `blocks`
- `repoState`: final `but status --json` snapshot (best effort)
- `watchedFiles`: before/after hashes and `changed` booleans
- `resultMeta`: runner metadata (`isError`, turns, duration, etc.)
- `error`: provider-level error when setup/runner fails

Set `keep_fixtures: true` in `promptfooconfig.yaml` while debugging to inspect fixture state.

## Failure-Driven Tuning Loop

Use `pnpm run eval:natural:diagnose` while iterating on the skill and assertions.
It prints:

- threshold/KPI summary from `natural-metrics`
- failed-row diagnostics from `natural-failures`

Common missing-signal labels:

- `missing_plan_command`
- `plan_not_before_edit`
- `missing_discover_command_or_message`
- `missing_release_command`
- `claims_not_cleaned`
- `missing_autonomous_check`
- `missing_stack_dependency_check`
- `missing_dependency_coordination_message`
- `missing_stack_anchor_or_alignment`

Composite tuning loop:

1. `pnpm run eval:composite:repeat`
2. `pnpm run eval:composite:diagnose`
3. Tune one small `skill/SKILL.md` behavior at a time against top missing-signal labels

## Runner Stability Knobs

This harness now runs with conservative defaults:

- `evaluateOptions.maxConcurrency: 2` in promptfoo configs
- `evaluateOptions.timeoutMs: 240000` in promptfoo configs
- `evaluateOptions.maxEvalTimeMs: 7200000` in promptfoo configs
- Claude runner `--max-turns` (default `16`)
- provider hard timeout kill with `SIGKILL`
- one automatic retry for timeout/empty-output runner attempts
- promptfoo telemetry/update checks disabled in npm scripts

Useful overrides when tuning:

- `BUT_EVAL_RUNNER_TIMEOUT_MS=240000` (or higher)
- `BUT_EVAL_MAX_TURNS=20`
- `BUT_EVAL_RUNNER_RETRIES=2`
- `BUT_EVAL_RUNNER_RETRY_BACKOFF_MS=2000`
