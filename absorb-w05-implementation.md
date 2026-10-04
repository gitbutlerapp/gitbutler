# W05: Implement The Approved Atomic Path

Status: brief prepared, not executed. Prerequisite: explicit W04 design approval.
Allowed changes: approved implementation map and corresponding tests only.

Read [the plan](absorb-implementation-plan.md),
[the ledger](absorb-verification-ledger.md), W04's approval artifact, and current
applicable instructions/skills. Verify source identity, prerequisite evidence,
and ownership. A prepared W04 brief is not approval to start implementation.

## Direction

- Follow the approved small slices, starting with a W02/W03 failing test. Reuse
  existing selection interpretation and transaction/editor APIs. Preserve coupled
  selector order and original coordinate meaning across staged amendments.
- Include planning and selected-input eligibility in the atomic boundary. Do not
  silently omit blocked selections or wrap already-materializing calls and claim
  that the outer wrapper makes them atomic.
- Compose commit mappings and make staged changes visible to subsequent work
  without prematurely publishing refs, metadata, or other persistent state.
  Address database and finalization behavior exactly as approved in W04.
- Retain batching where proven safe. Remove obsolete singleton scheduling and
  implementation-detail tests only when replacement content tests preserve intent.
- Update old partial-success expectations explicitly to the new policy. Preserve
  applicable rejection-count and content coverage; document compatibility changes.

## Validation And Handoff

Immediately run the discriminating test after each substantive edit, repair the
same slice, then run affected existing absorb regressions and scoped required
formatting/lints. Do not weaken W02/W03 tests to obtain passing results.

Record exact commands, nonzero test counts, source identity, changed files, and
remaining gaps. Completion requires W02/W03 and prior relevant regressions to
pass, with no concealed side-effect escape. A new mechanism or safety limitation
outside the approved design returns to W04 rather than expanding implementation.

Update the ledger and hand off to W06. Stop without committing, pushing, changing
branches, publishing, or implementing deferred partial/resolution modes.

## Assignment Prompt

> Execute W05 only within the explicitly approved W04 map. Implement small slices
> against the failing tests, validate each immediately, and record evidence.
> Return design surprises to W04; stop after the implementation handoff.
