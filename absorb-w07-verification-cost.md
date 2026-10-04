# W07: Recovery, Generated Cases, And Cost

Status: brief prepared, not executed. Prerequisites: completed W05-W06 evidence.
Allowed changes: focused tests/benchmarks and proven local repairs within the
approved design. Split substantial work into bounded subpackets before expanding.

Read [the plan](absorb-implementation-plan.md),
[the ledger](absorb-verification-ledger.md), prerequisite evidence, and applicable
instructions/skills. Verify source identity and ownership. Load the performance
skill before modifying or running shell-based CLI performance scenarios.

## Direction

- Exercise undo, dry-run, changed source/stale plans, late errors, publication,
  and finalization using existing hooks first. Verify semantic state, not merely
  a returned error. Distinguish successful rollback from uncertain restoration.
- Close the supported-domain matrix with passing behavior or explicit safe
  rejection tests. Keep excluded optional partial/resolution modes deferred.
  Document crash limitations without claiming full ACID guarantees.
- Add bounded deterministic generated cases for a simple supported domain with
  an independent content oracle. Preserve seeds and minimized failures. Reuse
  existing tooling; new infrastructure requires approval, not speculative setup.
- Measure representative workload sizes, build profile, environment, baseline
  and new timings, plus amend/rebase/materialization counts. Use comparable runs
  and distinguish measured regressions from source-based cost estimates.
- Obtain a concrete performance budget/acceptance decision. Prefer agreement
  before acceptance runs; never invent a tolerance afterward to turn results green.
  If no budget exists, report measurements and leave acceptance pending.

## Validation And Handoff

Run each new focused test or benchmark after editing and preserve reproducible
commands, nonzero test counts, seeds, workload details, and results in the ledger.
Apply scoped formatting/lints. Local repairs must rerun the failed check and
affected regressions; architectural changes return to W04.

Completion requires matrix evidence and approved performance acceptance. Core
atomicity/recovery gaps block release and must remain visible. Record follow-ups
for nonblocking limitations, then hand off to W08 and stop. No commits, pushes,
branch changes, publication, or destructive live-workspace experiments.

## Assignment Prompt

> Execute W07 after W05-W06. Verify recovery and supported-domain boundaries,
> add bounded independent-oracle cases, and measure comparable cost. Record
> limitations and obtain performance acceptance; do not self-waive release gaps.
