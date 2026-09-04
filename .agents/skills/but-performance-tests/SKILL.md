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

1. Confirm requested operation and timing boundary.
2. Inspect closest existing scenarios plus `lib.sh` and `run.sh`.
3. Add or update only scenario's `setup.sh` and `test.sh` unless shared behavior is
   genuinely required.
4. Prefer real, representative GitButler repository state over toy data.
5. Keep setup, validation, and ID discovery outside timed operation.
6. Reuse documented helpers instead of creating parallel fixture, state, output, or
   timing mechanisms.
7. Update README's included-scenario list when adding or materially changing scenario.
8. Run README's shell validation and short single-scenario benchmark.

For usage or debugging questions, answer from current README and scripts. Do not edit
files unless user asks for changes.

## Guardrails

- Keep changes inside `crates/but/tests/performance/` unless task specifically requests
  related documentation or skill updates.
- Preserve two-file scenario contract.
- Do not add custom timing/statistics code; Hyperfine owns measurement.
- Do not include fixture setup or compilation in measured command.
- Do not run all scenarios when user requests or changes only one.
- Report fixture/setup limitations and unavailable validation explicitly.
