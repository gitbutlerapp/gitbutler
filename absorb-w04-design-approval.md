# W04: Technical Design And Approval

Status: brief prepared, not executed. Prerequisites: completed W01-W03 evidence.
Allowed changes: design documentation and ledger only.

Read [the plan](absorb-implementation-plan.md) and
[the ledger](absorb-verification-ledger.md), current applicable instructions, and
the prerequisite handoffs. Verify source identity and ownership before claiming
this packet. Missing evidence is a blocker, not permission to assume results.

## Direction

- Compare existing selection normalization/grouped execution with staged patches
  or composed graph edits. Choose the smallest design that satisfies the actual
  W02/W03 tests; do not presume bottom-up sorting or a transaction wrapper suffices.
- Put the design decision in the plan. Specify source snapshot and coordinates,
  coupled-selection identity/order, routing, staged-state visibility, commit
  mappings, safe batching, and the sole intended publication boundary.
- Include planning-created commits, eligibility, metadata/database writes, oplog,
  stale plans, undo, and caller compatibility. Distinguish ordinary rejection,
  publication/finalization errors, failed restoration, and crash limitations.
- Map each invariant to existing evidence or a required test. List exact intended
  files/symbols, reusable APIs, obsolete machinery, and small validated W05 slices.
  Flag new mechanisms explicitly for approval; do not implement prototypes here.
- Resolve the first-release technical decisions D1/D2/D3/D5 using the current
  ledger definitions. Keep approved product policy intact; escalate conflicts.

## Handoff Gate

Present the recommendation, alternatives rejected, risks, and remaining questions
to a human/maintainer. Record explicit approval with reviewer, date, artifact or
message reference, and approved scope. Preparing or recommending a design does
not approve it; do not self-certify or infer approval from earlier policy answers.

Validate document formatting, links, and consistency with the evidence. Until
approval and safety-critical answers exist, record W04 as blocked awaiting them.
Once approved, record the implementation map and next packet W05, then stop.
No production/test edits, commits, pushes, branch changes, or GitHub updates.

## Assignment Prompt

> Execute W04 only after verifying W01-W03 evidence. Produce the smallest
> evidence-backed design and implementation map, request explicit approval, and
> record its actual status in the ledger. Do not implement or approve it yourself.
