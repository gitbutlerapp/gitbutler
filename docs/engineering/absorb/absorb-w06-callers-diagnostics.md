# W06: Callers And Actionable Diagnostics

Status: brief prepared, not executed. Prerequisites: W05 and W01 caller inventory.
Allowed changes: affected adapters, diagnostics, documentation, and their tests.

Read [the plan](absorb-implementation-plan.md),
[the ledger](absorb-verification-ledger.md), prerequisite evidence, and applicable
instructions/skills. Verify source identity and claim this packet. Load UI/CLI
skills when working on those surfaces; do not broaden the task into a redesign.

## Direction

- Walk the actual W01 caller inventory: shared API, CLI/TUI, desktop, Lite, web,
  N-API, SDK, and docs as applicable. Record updated or source-backed unaffected
  status for each; generated types alone do not establish consumer correctness.
- Distinguish complete success, atomic rejection, and uncertain/failed restoration.
  Explain the blocker versus otherwise applicable work left unabsorbed. Include
  selected path/range, intended target, and known dependency where available.
- First reproduce F3's suspected fallback-diagnostic gap in an existing CLI
  fixture. Fix it only with supporting evidence, or narrow/withdraw the finding.
  Do not claim source-only suspicion is a demonstrated runtime failure.
- Keep output, exit status, API results, and generated contracts consistent. No
  adapter may report complete success for silently skipped selected input or hide
  failed restoration. Refresh required generated outputs using repo tooling.
- Recommend only verified commands with justified ancestry relationships. Where
  no safe repair is known, give accurate inspection guidance. Do not introduce
  automatic move/squash/discard, conflict-mode entry, or implicit partial success.

## Validation And Handoff

Validate each changed caller with focused tests immediately; include known,
unknown, and fallback dependencies and relevant machine-readable outcomes. Run
required scoped formatting/lints. Record actual commands, counts, source state,
contract changes, and updated/unaffected evidence in the ledger.

Completion requires the caller inventory to be accounted for and diagnostic
claims to be supported. Return architecture/policy conflicts to W04. Hand off
to W07 and stop; no commits, pushes, branch changes, or GitHub updates.

## Assignment Prompt

> Execute W06 against the W01 caller inventory and W05 behavior. Verify F3 before
> fixing it, propagate honest outcomes, and test diagnostics and consumers.
> Record updated/unaffected surfaces and stop without publication.
