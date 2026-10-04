# W08: Independent Review And Release Readiness

Status: brief prepared, not executed. Prerequisite: completed W07 evidence.
Allowed changes: ledger/docs and review-driven fixes with repeated validation.
Publication requires separate explicit authorization.

Read [the plan](absorb-implementation-plan.md),
[the ledger](absorb-verification-ledger.md), prerequisite evidence, and current
applicable instructions, including the review/prepush skill. Verify the intended
source state and review scope; do not switch PRs based on an unrelated attachment.

## Direction

- Arrange a separate review pass over the full intended change, not merely the
  latest patch. Supply policy, design, test evidence, and known limitations. Another
  agent or human may review, but do not spawn an agent without authorization.
  If independent review is unavailable, record the gate as pending.
- Review selection fidelity, atomicity through planning/finalization, caller
  contracts, diagnostics, recovery, and cost. Resolve core findings with evidence;
  an explicit scope decision cannot silently contradict approved product policy.
- Run relevant crate suites, strict lints, and formatting against the actual
  release candidate. Record dirty/applied changes and limits of isolation. After
  fixes, rerun focused checks and affected required gates on the new candidate.
- Bring the algorithm explanation and local PR-text draft into agreement with
  the implementation. Label deferred modes and crash limitations accurately;
  do not advertise unimplemented resolution or promise zero possible bugs.

## Publication Gate And Handoff

Without explicit publication permission, stop with review acceptance and
reproducible release-readiness evidence. Preparing W08 does not authorize commits,
pushes, branch changes, GitHub edits, or resolving review threads.

If publication is separately authorized, recheck source identity, applicable
instructions, intended changes, and approval scope. Use the GitButler workflow for
version-control writes and preserve unrelated work. Verify remote head, relevant
checks, and published content before resolving threads; a successful local push
alone does not establish passing CI or accepted review.

Record candidate identity, reviewer/artifact, validation, remaining limitations,
and publication outcome when applicable. Mark readiness separately from publication
so an unrequested push is not treated as missing implementation work. Stop after
the authorized handoff; do not begin deferred features automatically.

## Assignment Prompt

> Execute W08 after W07: obtain independent review, validate the actual candidate,
> and prepare accurate release documentation. Record readiness and blockers.
> Publish or resolve GitHub threads only with separate explicit authorization.
