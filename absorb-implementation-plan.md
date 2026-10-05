# Reliable Absorb: Design And Delivery Plan

Status: product policies approved on 2026-10-04; implementation design and evidence
remain pending. See the approved policy below and the decision ledger.
Created: 2026-10-04.
Scope: [PR #16181](https://github.com/gitbutlerapp/gitbutler/pull/16181), reviewed
at `64436487e7c57bdfde3da4785f1e9d6e52c38d50`, and explicitly approved follow-ups.
Track completion and evidence in the [ledger](absorb-verification-ledger.md).

For agent execution, start with [the work packets](#agent-work-packets), not the
phase headings. Phases explain the strategy; packets define executable scope.

## Read This First

Keep the existing implementation foundations and repair the violated contracts.
Do not start a replacement project or assume a new branch makes the algorithm
safer. Use the existing branch for focused fixes; separate product-level conflict
workflow changes if they require broader APIs or UI work. Do not rewrite published
history, split commits, or publish changes without authorization.

This document proposes work; it does not establish that the proposed algorithm
is correct or that the conflict workflow already supports absorb. Review Phase 0
before committing to an implementation shape. Passing tests reduce risk, but
neither exhaustive scenario enumeration nor a rewrite guarantees bug-free code.

## Plain-Language Explanation

Imagine a document edited in several saved versions. Absorb tries to put today's
small corrections into the earlier versions that they belong with, then rebuilds
the later versions on top. Three separate questions must be answered correctly:

1. What exactly did the user select? Removing an old line and adding its new
   replacement can be two selections that only make sense together.
2. Which earlier version should receive it? Overlap and dependency information
   help, but cannot always establish the author's intended destination.
3. Can later versions still be replayed? An edit can be correctly selected and
   routed yet conflict with a later edit of the same material.

Line numbers are addresses in a particular version, not permanent identities.
Deleting a line changes later addresses. Applying independent edits from the
bottom upwards can avoid some shifts, but does not make arbitrary selections
independent or preserve every diff boundary after recomputation.

The current PR splits every selector into a separate amendment. This breaks the
existing interpretation of paired selections and can commit unselected content.
That is a correctness defect, not an acceptable limitation of absorb.

A genuine conflict is different. For example, an earlier commit sets a timeout
to 10, a later commit changes it to 20, and today's selected edit asks to change
the earlier value to 15. Replaying the later change may require a decision about
which value is intended. There may be no uniquely correct automatic answer.
Likewise, successful text merging alone does not prove semantic intent.

Reliable absorb should never knowingly guess through ambiguity or silently
change the selection. Its supported outcomes should be explicit: completed,
rejected safely with an explanation, or paused in a supported resolution workflow.
Always succeeding without user input is not a defensible guarantee.

The approved first-release behavior is to abort unchanged on a conflict and
explain supported next steps. Interactive resolution is a separate, explicitly
initiated follow-up; automatically entering it is not the default absorb contract.
Whether the existing workflow can support that follow-up remains unverified.

## Approved Product Policy

User decisions collected through VS Code questions on 2026-10-04:

- Default absorb is atomic for the entire invocation, across independent branches,
  from planning through publication. Every selected change must be applied or none
  is published. Blank commits created during planning are inside this boundary.
- Ambiguous destinations fail the invocation and request an explicit target or
  narrower selection. Immutable/landed or otherwise ineligible selected changes
  cannot be silently skipped while reporting complete success.
- On expected rejection or operational failure, preserve user-visible state and
  explain the selection, intended target, and known blocker. Suggest verified
  recovery commands where available; otherwise provide accurate inspection steps.
  Never invent a remedy or automatically move, squash, or discard changes.
- Partial success is permitted only as a separately specified, explicit opt-in
  mode. It may ship later; do not retain implicit partial behavior in the meantime.
- The release bar is operational atomicity with audited crash limitations, not a
  claim of full crash-proof ACID behavior. Audit finalization and database writes;
  do not report successful rollback if restoration is uncertain or failed.
- Conflict-resolution integration may follow later and must be explicitly
  initiated. It is not a prerequisite for releasing the verified atomic path.

These decisions approve behavior, not a particular implementation. The existing
transaction abstraction is a candidate: it stages graph work and supports commit
mapping, but explicitly excludes project-database rows and is not fully ACID.
Do not just wrap calls that already materialize each amendment in a transaction.
Use transaction-aware operations or compose graph edits before publication, and
audit all side effects including planning, metadata, index, worktree, and oplog.

## Correctness Contract To Approve

- Selection fidelity: only selected content is introduced into the chosen target;
  coupled old/new selections retain their shared meaning and relative order.
- Routing fidelity: dependency evidence selects a justified target; ambiguity is
  reported, not resolved by incidental stack order. Define explicit-target behavior.
- Stable interpretation: selections are interpreted against a known source state;
  changed source state is detected or safely revalidated before application.
- Preservation: normal successful absorption preserves working-tree content and
  leaves the correct residual diff. Specify index, metadata, and ref invariants too.
- History integrity: descendants and refs are rewritten consistently, using exact
  commit identities and composed replacement mappings, not branch labels alone.
- Honest outcomes: default failure publishes no selected changes; distinguish
  blocking/rejected selections from otherwise applicable changes rolled back.
  Preserve count units where retained and explicitly review compatibility of the
  existing count-returning API with the approved atomic error contract.
- Recovery: undo restores the intended pre-operation state; dry-run has no
  persistent effects. Resolution mode has a separately specified state contract.
- Supported domain: explicitly classify renames, binary files, mode changes,
  non-UTF-8 paths, linked worktrees, immutable commits, and merge histories as
  supported, safely rejected, or deferred. Do not silently expand support.

## Phase 0: Review The Explanation And Contracts

1. Read current root and applicable scoped instructions and workflow skills.
2. Trace selection conversion, routing, amendment, materialization, and recovery
   at the current head. Separate unchanged limitations from introduced defects.
3. Validate the explanation above against real examples, including a legitimate
   later-commit conflict. Check existing conflict-resolution entry points in each
   relevant surface rather than generalizing one amend refusal to the whole app.
4. Record decisions D1-D5 in the ledger: selection semantics, routing ambiguity,
   partial failure, conflict handoff, and supported scope.
5. Obtain human/maintainer review of the explanation and contract before new
   mechanisms or public API changes. Unknowns remain visibly open.

Exit: approved contract and bounded implementation scope; no unresolved safety
decision is hidden behind an implementation assumption.

## Phase 1: Establish Failing Regressions

1. Add fixture-backed public-operation tests for paired replacements and multiple
   old-side deletions. Observe failures on the reviewed implementation first.
2. Cover mixed full/partial selections and paired selectors whose dependency
   lookups could choose different targets. Exercise planning plus execution, not
   only a hand-constructed application plan.
3. Reproduce the suspected fallback descendant-hint issue separately; withdraw or
   narrow that finding if the reachable behavior does not support it.
4. Preserve existing ordering, boundary, ambiguity, and rejection-count tests.
5. Assert exact target blobs, relevant descendant/ref state, residual changes,
   worktree preservation, and rejection counts. Use snapshots where informative.

Exit: minimal failing cases and explicit expected outcomes, not tests that merely
assert the new scheduler's internal arrangement.

## Phase 2: Select And Implement A Bounded Design

1. Inspect existing patch-selection normalization and graph editor APIs for reuse.
   Do not duplicate partial-patch semantics or invent a parallel history engine.
2. Evaluate this candidate: interpret selections against one source snapshot,
   preserve groups belonging to one underlying diff hunk, retain within-group
   order, and schedule independent groups bottom-up per path.
3. Verify grouping across target assignments, old/new coordinate spaces, context
   settings, adjacent edits, and changed diff segmentation after each amendment.
   Grouping alone is not assumed sufficient. If it fails, evaluate normalized
   patches anchored to original blobs or existing composed editor operations.
4. Document the selected design, assumptions, and why rejected alternatives fail.
   Propose any new module, API, or traversal before building it.
5. Make the smallest correction, remove obsolete singleton logic and assertions,
   and run the failing test immediately after each substantive edit.
6. Preserve safe batching, especially independent files with one target. Keep
   commit mapping and count semantics intact. Fix diagnostic hints independently.

Exit: regression tests pass without weakened assertions; design review can explain
why remaining groups do not invalidate one another.

## Phase 3: Strengthen Evidence And Measure Cost

1. Complete the ledger's risk-based test matrix. Use pairwise interactions plus
   targeted higher-order cases; do not attempt every Cartesian combination.
2. Add deterministic generated/property tests with an independent content oracle
   for a deliberately simple supported domain. Expected content must not call the
   scheduler under test. Record seeds and minimize counterexamples into fixtures.
3. Check permutation invariance only for genuinely independent groups. Check
   exact committed content, worktree preservation, and complete residual changes.
4. Test rejection and fatal-error state, undo, dry-run, and source-state changes
   according to the approved contract. Add fault injection only if existing test
   hooks cannot exercise a meaningful failure boundary.
   Include late rejection after earlier staged successes, planning-created commits,
   database writes, publication errors, and truthful reporting of failed recovery.
5. Measure amend/rebase/materialization counts and representative runtime against
   the baseline for many hunks, files, and descendants. Record an agreed budget
   before claiming performance acceptance; correctness takes priority.

Exit: reproducible correctness evidence and an explicit performance decision.

## Phase 4: Conflict-Resolution Integration

Approved as a separate follow-up, not a first-release gate. Default absorb aborts
unchanged; any resolution action is explicitly initiated. Partial-success mode
also needs its own result and recovery contract before being offered.

1. Inventory existing conflict representation, persistence, UI/CLI entry points,
   continue/abort operations, and undo behavior. Reuse them where suitable.
2. Specify a resumable operation state: original source and targets, completed and
   pending work, rejected groups, rewritten IDs, and the user's resolution.
   Persist only what the established workflow needs; avoid speculative machinery.
3. Design the explicitly initiated handoff and noninteractive behavior, preserving
   the approved default of unchanged abort. Never block an unattended CLI waiting
   for an editor or silently choose a resolution.
4. Verify that abort restores the agreed state; continue applies remaining work
   without duplication or stale selectors; cancellation, restart, source changes,
   and subsequent conflicts have defined behavior.
5. Document that conflict markers or resolution edits may intentionally alter the
   worktree in this mode, unlike ordinary successful absorb.

Exit: reviewed and tested recovery workflow, or an explicitly deferred follow-up
with safe interim behavior and product approval. This phase may be a separate PR;
it must not be silently marked complete to ship the correctness repair.

## Phase 5: Review And Publish

1. Review the full diff independently of passing tests, especially selector
   contracts, exact ancestry, ownership, permissions, and failure behavior.
2. Audit desktop, Lite, CLI/TUI, N-API, SDK, and docs: update affected contracts or
   record why each is unaffected. Regenerate SDK output only when required.
3. Run focused tests first, then relevant crate suites, strict linting, formatting,
   and required CI gates. Record exact revision and local modifications for runs.
4. Update the PR description with the implemented algorithm, guarantees, supported
   scope, known limitations, and evidence. Do not present a proposal as shipped.
5. Consider moving agent-workflow changes to a separate PR, with authorization.
   Publish only intended files; respond to findings only after the fix is pushed.

Exit: review approval and release criteria satisfied, or named blockers. Track
post-merge failures as minimized regressions; keep this ledger as historical
evidence and put lasting behavior documentation beside the owning feature.

## Implementation Anchors

- [Absorb planner and executor](crates/but-api/src/legacy/absorb.rs)
- [Selector conversion and application](crates/but-core/src/tree/mod.rs)
- [Existing selection contract tests](crates/but-core/src/tree/tests.rs)
- [DiffSpec conversion and commit mapping](crates/but-hunk-assignment/src/lib.rs)
- [Commit amendment](crates/but-workspace/src/commit/commit_amend.rs)
- [Materialization](crates/but-api/src/workspace_state.rs)
- [Dependency ranges](crates/but-hunk-dependency/src/ranges/mod.rs)
- [Rejection explanations](crates/but/src/utils/rejection.rs)
- [Graph and workspace guidance](crates/WORKSPACE_MODEL.md)

## Agent Work Packets

### Execution Protocol

1. Read this plan, the ledger, current root instructions, applicable scoped
   instructions, and required skills. Treat the approved product policy as fixed;
   do not ask the user to approve it again unless new evidence forces a change.
2. Start with the ledger's next packet. Verify its prerequisites and current source
   state. Record the actual commit, applied branches, and relevant dirty changes;
   do not assume the historical PR head is still current. Preserve others' work.
3. Claim one packet in the ledger before substantive work. No parallel agents
   editing the same slice. Delegation requires separate authorization.
4. Use the listed anchors as entry points, not as permission for broad exploration.
   Follow the controlling behavior one local boundary at a time. Commands below
   are starting points; verify test names against the current tree.
5. Record expected failures for test-only packets. A red test is valid handoff
   evidence, but not a release-ready state. Never ignore/delete it or change its
   expectation merely to regain green. Run narrow checks immediately after edits.
6. A completed packet needs concrete artifacts and evidence in the ledger. Record
   unresolved issues and exact next action. If findings invalidate the design,
   block the packet and return to the design gate, not an improvised architecture.
7. Stop after the assigned packet unless explicitly authorized to continue through
   ready packets. Always stop at approval gates. This plan does not authorize
   commits, pushes, branch creation, PR changes, or destructive workspace actions.

Product-policy approval already permits investigation and failing test creation.
Production implementation requires W04 design approval. Thus Phases 0 and 1 may
overlap for evidence gathering; there is no requirement to prove the algorithm
before writing the tests that discriminate between designs.

### W00: Establish The Current Baseline

Prerequisites: none. Allowed changes: ledger only.

Execute [the dedicated W00 brief](absorb-w00-baseline.md) for step-by-step
commands, test-discovery guidance, evidence requirements, and stop conditions.
The summary below defines scope; live completion state remains in the ledger.

- Identify the actual implementation branch and base, exact revision, applied
  branches, and dirty files. The historical absorb PR is #16181; a later chat
  attachment mentioned #16182 without establishing its relationship. Do not
  switch scope or publish to either number based solely on an attachment.
- Verify the implementation anchors and available fixture/test commands. Record
  the current baseline for `cargo test -p but-api legacy::absorb::tests` and
  `cargo test -p but --test but command::absorb` when available.
- Attribute existing failures without fixing unrelated work. Propose an isolated
  validation approach if unrelated changes prevent trustworthy results; do not
  change branches or create worktrees without authorization.

Exit: ledger contains reproducible source identity, baseline outcomes, and the
correct next task. Stop if the intended source cannot be identified safely.

### W01: Audit State Boundaries And Caller Contracts

Prerequisite: W00. Allowed changes: plan and ledger only; no production edits.

Execute [the dedicated W01 brief](absorb-w01-state-audit.md) for investigation
boundaries, required audit artifacts, and the test/design handoff. Live state
remains in the ledger; preparing the brief does not satisfy W00.

Start at `absorption_plan_with_perm`, `ensure_target_commit`, `absorb_with_perm`,
`commit_amend_only_impl`, and
[transaction entry points](crates/but-transaction/src/lib.rs). Trace the CLI
planner, landed-target filtering, and snapshot timing too.

- Record a side-effect table in the ledger: operation/symbol, state read or
  written, when persistence occurs, transaction coverage, and required test.
  Include refs/HEAD, index, worktree, metadata, database, objects, oplog, caches,
  and notifications where applicable. Distinguish unreachable object creation
  from changes to user-visible history; do not demand byte-identical object stores.
- Determine how later transaction steps see prior staged amendments and remaining
  worktree changes. An in-memory rebase alone does not prove correct diff inputs.
- Inventory actual API and application callers, including separate plan/apply
  requests. Specify stale-plan handling and identify return/error compatibility
  implications of replacing partial-success counts with atomic failure.
- Classify supported input types and exact safety unknowns. Record source-backed
  facts versus hypotheses; do not implement a transaction wrapper at this stage.

Exit: side-effect and caller tables identify a proposed atomic boundary and every
known escape from it. Unknown coverage becomes an explicit blocker/test, not an
assumption. These artifacts inform W03 and W04.

### W02: Reproduce Selection Failures

Prerequisite: W00. Allowed changes: existing absorb API tests and owning-crate
scenario fixtures; shared production selection logic stays unchanged.

Execute [the dedicated W02 brief](absorb-w02-selector-regressions.md) for concrete
selection cases, expected content, validation, and evidence requirements. W01
findings are useful input but are not an additional prerequisite.

- First reproduce the review example: old/new line-5 selections in a ten-line
  replacement. Expected committed content replaces only the selected line.
- Add multiple old-only selections, then mixed full/partial selections. Derive
  expected blobs independently from the scheduler; show exact residual changes.
- Exercise planner plus executor for coupled selections and potentially different
  target assignments. Distinguish a planner defect from an executor defect.
- Use `but_testsupport::writable_scenario` and retain its guard. Keep existing
  regressions intact. Run each new named test immediately and record its failure
  before any production fix, then run the existing absorb tests for comparison.

Exit: F1/F2 are confirmed with named failing tests, or narrowed/withdrawn with
evidence. Preserve worktree content and inspect target/descendant blobs and refs.
Do not assert that all future execution steps must contain exactly one selector.

### W03: Specify Atomicity In Executable Tests

Prerequisites: W01. Allowed changes: fixture-backed API/CLI tests and test helpers
only where existing helpers cannot express the required state comparison.

Execute [the dedicated W03 brief](absorb-w03-atomicity-tests.md) for concrete
failure scenarios, semantic state comparisons, success/dry-run counterparts,
and rules for attributing expected policy failures.

- Reproduce a later rejection after an earlier otherwise successful amendment.
  Assert all selected changes remain unabsorbed and all relevant refs unchanged.
- Cover cross-branch failure, planning-created blank commits, and a selected
  immutable/landed target. No silent omission may satisfy the test.
- Capture before/after user-visible state using established helpers: worktree
  bytes/modes, index state, refs, relevant metadata/database records, and oplog.
  Avoid unstable full-directory snapshots and timestamps as primary oracles.
- Add success and dry-run counterparts; identify error categories that need a
  finalization test later. Record which failures are expected on the old policy.

Exit: tests define the new policy unambiguously, with failures attributed to
pre-existing partial behavior rather than presented as PR-introduced regressions.

### W04: Review And Approve The Technical Design

Direction brief: [W04 design approval](absorb-w04-design-approval.md).

Prerequisites: W01-W03. Allowed changes: design and ledger only.

- Compare existing selection normalization plus grouped execution against staged
  normalized patches/composed graph edits. Select the smallest design that meets
  the tests; bottom-up grouping is not mandatory or presumed sufficient.
- Write a short design decision in this plan: data flow, source snapshot and
  coordinate meaning, group identity, routing, staged-state visibility, commit
  mapping, materialization boundary, rejection handling, and safe batching.
- Specify database and publication-error handling, undo, stale plans, and caller
  compatibility. Define what may remain after a crash versus ordinary rejection.
- List intended symbols/files to change, old machinery to remove, and tests
  proving each invariant. Split W05 further here if the approved implementation
  cannot fit a small, independently validated slice.
- Obtain explicit human/maintainer design approval; record reviewer and artifact.
  Do not self-certify approval or infer it from the product-policy answers.

#### W04 design recommendation (approval required)

**Recommendation:** interpret the complete request against one immutable source
snapshot, normalize selections into groups that retain the underlying diff-hunk
identity and within-group order, route complete groups before mutation, and stage
all graph/history and user-visible state changes until one publication boundary.
Use the existing transaction-aware graph/editor and materialization APIs rather
than wrapping the current per-hunk `commit_amend_only_impl` loop. A group is the
smallest application unit: coupled old/new selectors cannot be split; genuinely
independent groups may be batched only when their source paths and staged target
state do not overlap.

The source snapshot includes the relevant target commit trees, worktree/index
view, hunk headers, dependency/assignment rows, project/workspace metadata, and
an operation revision. Hunk coordinates remain addresses in that snapshot, not
live coordinates after an earlier amendment. Shared planning validates routing
and target identity; staged application validates amend eligibility before
publication. Surface policy remains explicit: the CLI rejects selected landed
targets unless `--allow-merged` is set, while the shared planner does not itself
classify landed status. A plan sent across the API boundary carries the source
revision and relevant preconditions; apply revalidates them and aborts stale
plans without publishing.

Staged execution builds one graph/editor state and a source-consumption map. It
produces replacement commit mappings for descendants and workspace refs, stages
assignment/database changes in the same logical operation, and materializes once
after all groups succeed. The sole ordinary publication boundary is the final
materialization plus coordinated metadata/index/worktree/oplog finalization. An
expected rejection, stale precondition, ambiguity, or selected ineligible target
discards staged state and returns an atomic failure. A fatal error after
publication is reported as a finalization/restoration failure; the release claim
is operational atomicity for ordinary rejection, not crash-proof ACID.

Planning-created blank commits are staged graph nodes, never immediate refs.
Dry-run executes planning and validation against the same staged model, emits
the preview, and drops it without refs, objects becoming reachable, database
rows, index/worktree changes, or oplog entries. Undo records one operation
snapshot only after successful publication; snapshot failure is surfaced as a
finalization limitation rather than silently presented as a fully recoverable
operation.

Rejected alternatives: retaining singleton materialization with bottom-up
sorting does not preserve coupled selectors or prevent repeated source
consumption (W02 F1/F2). A wrapper around the existing eager amend calls does
not roll back refs, worktree/index, assignments, blank commits, or oplog state
(W03 A1-A4). `but-transaction` alone is insufficient because its documented
best-effort boundary excludes project-database rows. Implicit partial success
and automatic conflict entry violate approved policy.

**Bounded W05 slices, pending approval:**

1. Add source-revision/precondition capture and grouped staged-plan types at the
   absorb planning boundary; prove stale-plan and routing/eligibility rejection
   before publication.
2. Replace singleton executor materialization with one grouped graph/editor
   composition and explicit commit mapping; prove W02 selector fidelity and A5
   success controls.
3. Stage assignment/database and blank-commit effects, then add the single
   publication/finalization boundary; prove W03 A1-A4 and A6 expected-red tests
   become unchanged-state passes while preserving dry-run and undo contracts.
4. Update only the directly affected API/CLI result contract after the above
   slices pass; defer caller/UI work to W06.

Intended primary files/symbols are `crates/but-api/src/legacy/absorb.rs`
(`absorption_plan_with_perm`, `ensure_target_commit`, `absorb_with_perm`), the
existing graph/editor or transaction-aware materialization APIs under
`crates/but-transaction`, `crates/but-rebase`, and `crates/but-workspace`, plus
their focused tests. No new production fault-injection mechanism, dependency,
generated binding, or parallel history engine is approved by this recommendation.

Exit: approved D1 and technical resolutions for D2/D3/D5 needed by the first
release. Stop here if approval or a safety-critical answer is missing.

### W05: Implement The Approved Atomic Absorb Path

Direction brief: [W05 implementation](absorb-w05-implementation.md).

Prerequisite: W04 approval. Allowed changes: only the approved implementation map
and corresponding tests. Existing skills/scoped instructions remain mandatory.

- Follow the approved slices, beginning with the smallest correction discriminated
  by W02/W03. Reuse patch interpretation and transaction/editor APIs. Do not put
  existing per-step materializations inside an outer wrapper and call it atomic.
- Include planning and eligibility validation in the safe boundary. Reject any
  selected failure before publication and preserve within-group selector order.
- Verify staged amendments consume exactly their intended source changes; use
  composed replacement mappings and retain batching wherever proven safe.
- Update old partial-success expectations explicitly to the approved policy,
  retaining their rejection-count/content intent where applicable. Do not claim
  backwards-compatible behavior when the outcome contract deliberately changes.
- Run the exact failing tests after each slice, then existing absorb tests. Remove
  obsolete singleton code/tests only once replacement coverage demonstrates intent.

Exit: W02/W03 tests and prior relevant regressions pass; no unresolved side-effect
escape is concealed. Unexpected design limitations return to W04.

### W06: Make Failures Actionable Across Callers

Direction brief: [W06 callers and diagnostics](absorb-w06-callers-diagnostics.md).

Prerequisites: W05 and W01 caller inventory. Allowed changes: affected adapters,
diagnostics, docs, and their tests; UI work must load its own scoped skills.

- Report atomic failure distinctly from partial success: blocker versus all work
  rolled back, selected path/range, target, and known dependency where available.
- Reproduce F3 through an existing CLI fixture first. Fix or withdraw the finding;
  never suggest ancestry-changing recovery without verified relationship checks.
- Update API/SDK consumers and CLI output/exit status consistently. Test that no
  adapter prints success for skipped input or hides a failed restoration.
- Suggest only existing, observed commands. When no safe repair is known, provide
  precise inspection guidance. No new automatic resolution or partial mode.

Exit: caller inventory has evidence for updated/unaffected surfaces; diagnostic
tests cover known, unknown, and fallback dependencies. Required generated outputs
are refreshed when contracts change.

### W07: Verify Recovery, Generated Cases, And Cost

Direction brief: [W07 verification and cost](absorb-w07-verification-cost.md).

Prerequisites: W05-W06. Allowed changes: focused tests/benchmarks and proven local
repairs within the approved design. Split into subpackets before large expansion.

- Cover undo, dry-run, source changes, late operational errors, and publication
  failures using existing hooks first. Distinguish rollback success from uncertain
  state. Record exactly which crash behavior remains outside the guarantee.
- Complete the supported-domain matrix; use explicit rejection tests for excluded
  cases. Deferred optional modes do not need implementation to pass this gate.
- Add bounded deterministic generated tests for a simple domain with an independent
  oracle. Preserve seeds/minimized fixtures; avoid adding a framework unnecessarily.
- Record workload sizes, build profile, environment, baseline and new timings, and
  amend/rebase/materialization counts. Obtain a concrete performance acceptance
  decision; never invent a tolerance after seeing results to mark the gate green.

Exit: evidence covers the matrix and approved performance budget, with named gaps
blocking release where they concern the core guarantee.

### W08: Independent Review And Release Readiness

Direction brief: [W08 review and release](absorb-w08-review-release.md).

Prerequisite: W07. Allowed changes: ledger/documentation and review-driven fixes
that repeat focused validation. Publication still requires explicit authorization.

- Have a separate review pass examine the full intended diff and invariants, not
  just the latest patch. It may be another agent/user; do not spawn one implicitly.
- Run relevant crate suites, strict lints and formatting against the actual release
  state. Record applied/dirty changes and any limitation of isolation.
- Update the algorithm explanation and PR text draft to match what was built.
  Resolve every core finding or obtain an explicit scope decision; deferred
  partial/resolution modes remain labeled follow-ups, not completed functionality.
- Before authorized publication, recheck head and instructions, use GitButler for
  version-control writes, and include only intended changes. Verify remote results
  before resolving threads. Without permission, stop at release-ready evidence.

Exit: accepted review and reproducible validation, plus authorized publication
evidence if publication was requested. No blanket claim of zero possible bugs.

### Deferred Packets

Explicit partial-success mode and interactive conflict resolution are separate
future tasks requiring their own policy detail, tests, and approval. They do not
block W08 under the approved staged-delivery policy. Do not begin them merely
because all currently ready tasks are complete.

### Copyable Handoff Prompt

> Read AGENTS.md, absorb-implementation-plan.md, and
> absorb-verification-ledger.md. Follow applicable scoped instructions and skills.
> Execute only the ledger's next ready work packet, starting with its prerequisites.
> Preserve unrelated work and approved policies. Do not commit, push, change
> branches, update GitHub, or begin deferred features without explicit permission.
> Record exact evidence, changed files, unresolved questions, and the next packet
> in the ledger. Stop at design/approval gates rather than inventing approval.
> End with a concise handoff stating what passed, what failed, and what comes next.
