---
name: but-performance-tests
description: Use when creating, changing, running, or debugging shell-based `but` CLI performance scenarios under `crates/but/tests/performance`, including Hyperfine runs, fixture setup, setup-to-test state, and output inspection.
---

# `but` CLI performance tests

Read [performance framework documentation](../../../crates/but/tests/performance/README.md)
completely before answering questions or changing performance scenarios. Inspect nearby
scenarios and current shared helpers as applicable; README is canonical source for
framework behavior, commands, helper APIs, and debugging options.

## Workflow for changes

1. Inspect closest existing scenarios plus `lib.sh` and `run.sh`.
2. Add or update only scenario's `setup.sh` and `test.sh` unless shared behavior is
   genuinely required.
3. Prefer real, representative GitButler repository state over toy data.
4. Keep setup, validation, and ID discovery outside timed operation.
5. Reuse documented helpers instead of creating parallel fixture, state, output, or
   timing mechanisms.
6. List every scenario in top-level performance README's included-scenario list, and
   update its entry when materially changing scenario.
7. Run README's shell validation and short single-scenario benchmark.

## Guardrails

- Preserve two-file scenario contract.
- Do not add custom timing/statistics code; Hyperfine owns measurement.
- Do not include fixture setup or compilation in measured command.
