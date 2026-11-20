---
name: but-performance-tests
description: Use when creating, changing, running, or debugging shell-based `but` CLI performance scenarios under `crates/but/tests/performance`, including Hyperfine runs, fixture setup, setup-to-test state, and output inspection.
---

# `but` CLI performance tests

[The performance README](../../../crates/but/tests/performance/README.md) documents the
framework: its commands, helpers, scenario layout and debugging options. Read all of it before
answering a question about the scenarios or changing one.

To add or change a scenario:

1. Start from the closest existing scenario, `lib.sh` and `run.sh`.
2. Change only the scenario's `setup.sh` and `test.sh`, unless shared behavior has to change.
   Reuse the README's helpers rather than adding fixture, state, output or timing mechanisms.
3. Use real, representative GitButler repository state, not toy data.
4. Keep fixture setup, compilation, validation and ID discovery out of `test.sh`, which
   Hyperfine times in full. Hyperfine owns measurement, so add no timing or statistics code.
5. List the scenario under the README's Included scenarios, and update its entry when the
   scenario changes materially.
6. Run the README's shell validation and a short single-scenario benchmark.
