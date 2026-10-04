# Absorb Decisions And Verification Ledger

Companion to the [design and phased plan](absorb-implementation-plan.md).
Updated: 2026-10-04. This is evidence tracking, not an automatic approval gate.

## Resume Here

Active packet: **W06 - Audit Callers And Diagnostics**. W05 completed the
approved atomic path and passed focused implementation review. Verify every
caller treats rejection and finalization failure honestly and gives actionable
diagnostics without weakening the atomic contract.
Later direction briefs: [W04 design](absorb-w04-design-approval.md),
[W05 implementation](absorb-w05-implementation.md),
[W06 callers](absorb-w06-callers-diagnostics.md),
[W07 verification](absorb-w07-verification-cost.md), and
[W08 release readiness](absorb-w08-review-release.md).
Current blocker to completion: API, CLI, Desktop, Lite, and transport callers
still need a consistent non-success contract for atomic rejection.

The old chat task list about committing skills and updating review threads belongs
to earlier completed publication work; it is not this implementation queue.

## Work Queue

Use states `pending`, `active`, `blocked`, or `done`. Only mark done with linked
evidence below. A failed reproduction test may complete a test-writing packet;
it does not complete its implementation fix or permit release.

| Packet                    | Prerequisites | State       | Evidence / next action                                                       |
| ------------------------- | ------------- | ----------- | ---------------------------------------------------------------------------- |
| W00 baseline              | None          | done        | Baseline evidence recorded below; next action is W01 state/transaction audit |
| W01 state/caller audit    | W00           | done        | Source-backed audit and W03/W04 inputs recorded below                        |
| W02 selector regressions  | W00           | done        | Five expected-red fixture regressions recorded below; W03 is next            |
| W03 atomicity regressions | W01           | done        | A1-A6 executable evidence recorded below; W04 design approval is next        |
| W04 design approval       | W01-W03       | done        | Original scope and blank-target amendment approved by the Author             |
| W05 implementation        | W04 approved  | done        | Atomic path, preconditions, finalization and focused validation recorded     |
| W06 diagnostics/callers   | W05           | active      | Make rejection/finalization non-success explicit across callers              |
| W07 verification/cost     | W05-W06       | pending     | Recovery, generated tests, performance decision                              |
| W08 review/release        | W07           | pending     | Independent review; publication separately authorized                        |

Recommended serial order is W00-W08. Dependencies permit W02 before W01 is
finished, but do not authorize concurrent edits. First-release scope excludes
optional partial mode and interactive resolution.

## Packet Handoff Template

Copy this into New Evidence when a packet starts, then complete it at handoff:

- Packet / owner / status / date:
- Prerequisites and approvals checked:
- Source revision, applied branches, relevant dirty state:
- Changed files and behavioral scope:
- Evidence: exact commands, test names, expected versus observed results:
- Design decisions or source references established:
- Remaining failures, uncertainty, or blockers:
- Next packet and first concrete action:

If interrupted, leave the packet active with the last completed command and any
running process identified. The next agent verifies state before repeating writes.

## W01 Audit Artifacts

### Side-Effect Table

| Operation / symbol                                                                                                                                                                                     | State affected                                                                                                                                                                                                                  | Persistence point                                                                                                                                                     | Transaction coverage                                                                                                    | Test / unresolved question                                                                                                |
| ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------- |
| [Planner](crates/but-api/src/legacy/absorb.rs) `absorption_plan_with_perm` -> [worktree diff](crates/but-api/src/diff.rs) `changes_in_worktree_with_perm`                                              | Reads HEAD worktree and index view; reconciles hunk assignments and dependencies                                                                                                                                                | Opens `db.immediate_transaction()`, calls `assignments_with_fallback`, which persists reconciled rows, then commits before routing completes                          | None. Project-database rows are explicitly outside `but-transaction`                                                    | W03 must compare assignment rows before planning and after planner error, dry-run, and later invocation failure           |
| [Planner target selection](crates/but-api/src/legacy/absorb.rs) `ensure_target_commit` -> [blank insert](crates/but-api/src/commit/insert_blank.rs)                                                    | May create a reachable blank commit and rewrite refs/worktree before the plan is returned                                                                                                                                       | Called with `DryRun::No`, then immediately materializes via `WorkspaceState::from_successful_rebase`                                                                  | None. Later routing failure, landed filtering, or dry-run return cannot roll it back                                    | W03 must cover planning-created blank commits followed by a blocker and dry-run                                           |
| [Executor](crates/but-api/src/legacy/absorb.rs) `absorb_with_perm`                                                                                                                                     | In-memory commit map, then target/descendant history and residual worktree state                                                                                                                                                | Singleton hunk steps each call `commit_amend_only_impl`, which materializes before the loop continues. Rejected specs are counted; an `Err` exits after earlier steps | None. It is not a transaction and does not defer publication                                                            | W03 must test late rejection and cross-branch failure from state captured before planning                                 |
| [Amend and materialization](crates/but-api/src/commit/amend.rs), [workspace amend](crates/but-workspace/src/commit/commit_amend.rs), [materializer](crates/but-rebase/src/graph_rebase/materialize.rs) | Objects, checkout/index/worktree, refs/HEAD, and refreshed workspace projection                                                                                                                                                 | Objects are persisted, then checkout runs, refs/HEAD are edited, then workspace refresh runs. Direct absorb has no restoration wrapper around these stages            | No outer transaction in current absorb                                                                                  | W07 needs fault-boundary evidence; a returned error cannot prove unchanged state after a publish stage                    |
| [Legacy snapshot](crates/but-api/src/legacy/absorb.rs), [CLI handler](crates/but/src/command/legacy/absorb.rs), [oplog](crates/gitbutler-oplog/src/oplog.rs)                                           | User-visible oplog commit, operations-log head, and reflog tracking                                                                                                                                                             | `create_snapshot` prepares and commits before execution; callers ignore snapshot errors with `.ok()`                                                                  | Not coupled to executor. Direct `absorb_with_perm` callers bypass it                                                    | W03 must compare oplog state on expected rejection; W04 must define truthful recovery after snapshot/finalization failure |
| [Transaction candidate](crates/but-transaction/src/lib.rs) `with_transaction_with_perm` / `Transaction::amend_commit`                                                                                  | In-memory rebase, mapped IDs, deferred metadata/ref/checkout work, one final materialization and oplog entry                                                                                                                    | Callback work remains staged; eager refs roll back on callback/finalization error                                                                                     | Candidate only; current absorb does not use it. It excludes project DB rows                                             | W04 must decide reuse only after a repeated-source-consumption composition test                                           |
| Metadata, caches, and notifications                                                                                                                                                                    | The direct normal amend path borrows metadata but has no explicit metadata setter. `Workspace::refresh_from_head` rebuilds from repo/meta/db. Watcher activity invalidates workspace, worktree, and `AbsorptionPlan` cache tags | No direct invalidation or watcher emission appears in audited absorb/amend sources                                                                                    | Transaction can stage metadata updates, but direct-path metadata behavior beyond inspected functions remains unverified | W03 compares relevant metadata; W06 verifies clients do not show success or stale plan after failure                      |

### Caller Table

| Caller / surface         | Current plan/apply and outcome contract                                                                                                                                          | Required change or unaffected rationale                                                                                                                                       | Evidence                                                                                                                                                                               |
| ------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Direct but-api and N-API | `absorption_plan` and `absorb` are separate `#[but_api(napi)]` functions. Plans have commit IDs and hunk headers but no source revision; apply returns a rejection-group `usize` | Atomic failure requires revalidation/versioning and a public return/error-contract decision                                                                                   | [API](crates/but-api/src/legacy/absorb.rs), [N-API linkage](crates/but-napi/src/lib.rs), [SDK declarations](packages/but-sdk/src/generated/graph/index.d.ts)                           |
| CLI `but absorb`         | One handler holds an internal exclusive guard while planning, filtering landed targets, displaying, snapshotting, and applying. It returns `Ok(())` after nonzero rejections     | Internal locking is not stale-plan protection against external edits. Landed entries are currently dropped, including mixed plans, contrary to approved selected-input policy | [CLI handler](crates/but/src/command/legacy/absorb.rs) and `absorb_json_reports_partially_skipped_merged_upstream_commits`                                                             |
| Desktop / Tauri          | Tauri registers separate plan/apply commands. Desktop fetches a plan, waits for confirmation, then submits the stored array; any resolved numeric result yields a success toast  | W06 must distinguish partial/atomic failure and revalidate old plans before submit                                                                                            | [Tauri registration](crates/gitbutler-tauri/src/main.rs), [endpoint](apps/desktop/src/lib/stacks/stackEndpoints.ts), [modal](apps/desktop/src/components/stack/AbsorbPlanModal.svelte) |
| Lite / Electron          | React Query fetches `window.lite.absorptionPlan`; `useAbsorb` later submits that array through `window.lite.absorb`, without inspecting a resolved rejection count               | W06 must specify stale-plan and atomic-failure behavior across query/mutation state                                                                                           | [query](apps/lite/ui/src/api/queries.ts), [mutation](apps/lite/ui/src/api/mutations.ts), [controls](apps/lite/ui/src/routes/project/$id/workspace/OperationControls.tsx)               |
| HTTP server              | Separate `/absorption_plan` and `/absorb` generated command routes                                                                                                               | Treat as split plan/apply transport until focused server coverage says otherwise                                                                                              | [server routes](crates/but-server/src/lib.rs)                                                                                                                                          |
| Web and TUI              | Targeted search found Web documentation only and no absorb invocation in the status TUI source subtree                                                                           | No application caller to change is established; recheck if a transport/action is added                                                                                        | Search scopes: `apps/web/**`, `crates/but/src/command/legacy/status/tui/**`                                                                                                            |

W04 records the approved design in the plan, with reviewer, date, intended file
map, and test-to-invariant mapping here. Approval of product policy is not design
approval. W07 records the performance workload, baseline, budget, and acceptance.

## Maintenance Rules

- Tick a task only when its exit evidence is recorded here. A passing neighboring
  test, an agent assertion, or a resolved review thread is not sufficient.
- Each evidence entry records date, revision plus relevant dirty state, command
  or review artifact, observed result, and limitations. Do not store secrets.
- Decisions record owner/reviewer, rationale, and approval. Supersede decisions
  explicitly rather than silently rewriting the historical rationale.
- Keep only this ledger and the plan until a genuinely separate deliverable is
  needed. Tests own executable examples; do not duplicate their full contents here.

## Phase Gates

- [x] Draft plain-language explanation, proposed contract, and phased plan.
      Evidence: the companion plan created 2026-10-04. Not yet approved.
- [ ] P0: review explanation and decide D1-D5 before implementation design approval.
- [ ] P1: demonstrate known selector regressions and investigate fallback hint.
- [ ] P2: approve and implement contract-preserving scheduling and safe batching.
- [ ] P3: complete risk-based verification and performance acceptance.
- [ ] P4: implement conflict handoff, or explicitly approve a tracked deferral.
- [ ] P5: complete cross-surface review, gates, accurate docs, and authorized release.

## Decision Register

| ID  | Decision needed                                                                    | Status   | Owner/reviewer and evidence                                                                  |
| --- | ---------------------------------------------------------------------------------- | -------- | -------------------------------------------------------------------------------------------- |
| D1  | Selection grouping, coordinate snapshot, and stable application model              | Proposed | W04 recommendation: immutable source snapshot, grouped staged application; approval required |
| D2  | Coupled selections with different proposed targets; ambiguity policy               | Proposed | Keep coupled groups intact; route or reject the complete group; approval required            |
| D3  | Partial rejection, fatal error, atomicity, undo, and dry-run contracts             | Proposed | One publication boundary; staged rejection; operational crash limits; approval required      |
| D4  | Conflict workflow reuse, automatic/opt-in handoff, continue/abort, unattended CLI  | Open     | Unassigned; requested direction is resolution support, mechanism unverified                  |
| D5  | Supported input domain, cross-surface scope, staged release and performance budget | Proposed | W05 bounded slices; W06 callers; W07 cost/domain gate; approval required                     |

## Approved Policy Decisions: 2026-10-04

Evidence: user answers to two VS Code question rounds in this session. These
approve requirements only; no implementation or verification gate is complete.

- [x] D2 policy: fail on ambiguous destinations; request explicit target or narrower
      selection. No guessing or silent skipping of selected changes.
- [x] D3 policy: entire invocation atomic by default, including planning-created
      commits and all selected changes across branches. Operational atomicity with
      audited crash limits; no false rollback claim. Technical side-effect audit open.
- [x] D3 optional mode: partial success only by explicit opt-in; may ship later.
      Its exact atomic unit, result format, and recovery contract remain undecided.
- [x] D4 first release: abort unchanged with verified guidance, not automatic
      conflict-mode entry. Accurate inspection steps are acceptable when no safe
      one-command remedy is known. Resolution integration is a separate follow-up.
- [x] D5 staging: atomic path can ship before optional partial/resolution modes.
      Supported domain, performance budget, and cross-surface compatibility remain open.

The decision register above remains open for the technical questions; these
approved policies constrain their answers. D1 still needs design review.

### W04 Design Recommendation - Approved Scope

- Packet / owner / status / date: W04 / GitHub Copilot / approved / 2026-10-04.
- Prerequisites checked: W01 audit, W02 selector regressions, and W03 A1-A6
  evidence are complete. Source identity remains the W03 revision and its
  preserved absorb test changes; no production implementation has started.
- Recommendation: use one immutable source snapshot, preserve coupled selector
  groups and within-group order, route and validate all groups before publication,
  compose staged graph/editor changes with explicit commit mappings, stage blank
  commits and assignment/database effects, and publish once after all groups
  succeed. Revalidate source revision/preconditions at split plan/apply callers.
- Rejected alternatives: singleton eager amendment with bottom-up sorting,
  wrapping eager calls in an outer transaction, implicit partial success, and
  automatic conflict entry. W02 F1/F2 and W03 A1-A4/A6 demonstrate why these do
  not satisfy the approved contract; the transaction audit shows project-database
  rows outside the existing best-effort transaction boundary.
- Proposed D1/D2/D3/D5 resolutions: the plan now specifies grouped staged
  application, coupled-group routing or explicit ambiguity failure, one ordinary
  publication boundary with operational crash limitations, and bounded W05/W06/W07
  scope. D4 remains the approved first-release abort-unchanged policy with
  resolution integration deferred.
- Intended W05 map: source/precondition capture; grouped graph/editor composition;
  staged assignment/blank-commit effects and final publication; then only the
  directly affected API/CLI result contract. Primary ownership is
  `crates/but-api/src/legacy/absorb.rs` and existing transaction/rebase/workspace
  materialization APIs. No new fault-injection mechanism or parallel history
  engine is proposed.
- Validation: plan and ledger formatting/link checks remain required; this packet
  changes design documentation only. No production or test behavior was changed.
- Approval record: the Author approved the recommendation on 2026-10-04. Approval
  scope is the current documented design and implementation map in this ledger
  and `absorb-implementation-plan.md`, including the bounded W05 slices and
  stated operational-atomicity/crash limitations. No additional mechanism,
  public contract, supported domain, or caller/UI scope is approved by this
  record; deviations return to W04 for review.

## Findings And Reproduction

- [x] F1: paired old/new selectors must remain coupled and preserve order.
      Confirmed by W02 paired replacement, mixed selection, single-target
      planner-plus-executor, and two-commit routing regressions. The first three
      demonstrate wrong target/residual content after singleton application; the
      two-commit case demonstrates planner splitting the pair before execution.
- [x] F2: multiple old-only selections must not run with stale coordinates.
      Confirmed by W02's two-selection fixture: original lines 2 and 7 select
      removal of lines 2 and 8 after the first materialized amendment.
- [ ] F3: fallback branch suspects may bypass descendant hint suppression.
      Prior evidence: code-path review only; not an end-to-end confirmed failure.
      Missing: reachable reproduction or documented withdrawal/narrowing.
- [ ] F4: quantify repeated rebase/materialization cost per selector.
      Prior evidence: inspected call path; no benchmark or acceptance budget yet.

## Test Matrix

For each checked row, add test names and evidence below. Reuse fixture harnesses.

- [x] Paired replacement, old-only, and mixed full/partial selections.
- [x] Several selections in one underlying hunk; several independent hunks.
- [ ] Same target, interleaved targets, ancestor/descendant targets, ambiguous targets.
- [x] Planner plus executor preserve coupled selections across target decisions.
- [ ] Insertions/deletions/replacements at beginning, middle, and end of file.
- [ ] Zero/default/nondefault context; adjacency and diff-boundary recomputation.
- [ ] Independent files and safe batching; exact intermediate target content.
- [ ] Original deletion-boundary case and nonmatching same-sized substitution.
- [ ] Partial rejections around successful rewrites; stable count units and mappings.
- [x] No unselected content committed; correct residual diff and preserved worktree.
- [ ] Ref/index/metadata preservation on failure; agreed partial-state behavior.
- [ ] Undo, dry-run, stale input, and unsupported-domain handling.
- [ ] Precise and fallback amend hints with ancestor, descendant, and independent refs.
- [ ] Deterministic generated cases with independent oracle and recorded seeds.
- [ ] Resolution continue/abort/cancel/restart and repeated conflict, if implemented.

## Prior Evidence: Baseline Only

Read-only review on 2026-10-04 at PR head
`64436487e7c57bdfde3da4785f1e9d6e52c38d50` reported:

| Command                                        | Result    |
| ---------------------------------------------- | --------- |
| `cargo test -p but-api legacy::absorb::tests`  | 4 passed  |
| `cargo test -p but-core to_additive_hunks`     | 7 passed  |
| `cargo test -p but-hunk-dependency --lib`      | 32 passed |
| `cargo test -p but --test but command::absorb` | 15 passed |
| `cargo test -p but --test but command::amend`  | 21 passed |

These 79 tests did not reproduce F1-F3 and do not establish their absence.
The worktree also contained another applied local-workspace branch and unrelated
forge/GitHub edits; this was not an isolated checkout of the published head.
Repeat release validation on the exact proposed changes and record contamination.
Earlier Vercel authorization failure was an infrastructure observation, not a
Rust correctness result; refresh CI when publishing, without assuming it persists.

## New Evidence

Source inspection on 2026-10-04 found that the pre-PR absorb implementation
(`git show fix-absorb-deletion-boundary~8:crates/but-api/src/legacy/absorb.rs`)
already amended groups sequentially and accumulated rejections without an outer
transaction. Partial application is pre-existing; singleton selector splitting
is introduced by the PR. This is source evidence, not a runtime reproduction.

[The transaction contract](crates/but-transaction/src/lib.rs) explicitly covers
rebases, refs and its metadata writes, excludes project-database rows, and calls
itself best-effort rather than fully ACID. Its callback can reject staged work
before materialization. This supports investigation of atomic absorb, but does
not prove all absorb side effects or final-publication errors are covered.

The earlier statement about accepting a partially completed absorb was an
unverified integration question, not a confirmed conflict-workflow defect.
No implementation, reproducer, benchmark, or complete conflict-workflow audit has
been completed under this plan yet. Append further entries in this form:

- Date / phase or finding ID:
- Revision and relevant local modifications:
- Test or artifact / exact command:
- Expected and observed outcome:
- Limitations / remaining action:
- Reviewer or approval, where required:

### W00 - Establish The Current Baseline

- Packet / owner / status / date: W00 / GitHub Copilot / done / 2026-10-04.
- Prerequisites and approvals checked: no prerequisites; product policy remains
  approved, while technical design approval at W04 is still required before
  production changes.
- Source revision, applied branches, relevant dirty state: checkout branch
  `gitbutler/workspace`, workspace and `HEAD`
  `8aa707cc9c95eb2650a1ab70f8496d03bbc5229f`; applied
  `fix-absorb-deletion-boundary` at
  `64436487e7c57bdfde3da4785f1e9d6e52c38d50` and
  `fix-cli-local-workspace-discovery` at
  `bcd48469bd88940245a6fadbec47801a288a3778`; `master`, `origin/master`,
  and their merge-base at `16315f27b97c8132931aa8d84b0b892d1d2a7a43`.
  `git status --short` and `target/debug/but status` showed modified
  `crates/but-forge/src/review.rs`, `crates/but-github/src/client.rs`, and
  `crates/but-github/src/lib.rs`; untracked absorb plan/ledger/brief files and
  `xen.d`. The relevant ledger was an uncommitted added file (`kuw:773`) in
  `target/debug/but diff`; these pre-existing notes and unrelated forge/GitHub
  and `xen.d` changes were preserved.
- Changed files and behavioral scope: W00 changed only this ledger; no
  production code, tests, fixtures, dependencies, branches, commits, or GitHub
  state were changed.
- Evidence: exact commands and observed results: - `pwd` and `git rev-parse --show-toplevel` both resolved to
  `/home/bkaindl/gh/gitbutler`. - `git rev-parse HEAD`, `git rev-parse --verify fix-absorb-deletion-boundary`,
  `git merge-base master fix-absorb-deletion-boundary`, and the corresponding
  post-run checks returned the IDs recorded above. - `target/debug/but status` showed both applied branches and the workspace
  dirty state recorded above. `target/debug/but diff kuw` identified the
  ledger change without inspecting unrelated forge/GitHub contents. - `cargo test -p but-api legacy::absorb::tests -- --list` selected 4 tests:
  `rejected_hunks_are_counted_once_per_original_commit_and_path`,
  `absorption_steps_preserve_per_path_descending_hunk_order`,
  `ambiguous_lock_targets_do_not_select_a_stack_by_workspace_order`, and
  `candidate_ending_at_a_deletion_point_includes_its_lock`. - `cargo test -p but --test but command::absorb -- --list` selected 15 tests,
  including source ambiguity/unresolvability, ancestor-before-descendant
  absorption, full and partial hunk selection, dry-run, merged-upstream and
  linked-worktree refusal, workspace refresh, independent-file concurrency,
  and single-branch behavior. - `cargo test -p but-api legacy::absorb::tests`: 4 passed, 0 failed, 0
  ignored; the separate API integration target selected 0 tests. - `cargo test -p but --test but command::absorb`: 15 passed, 0 failed, 0
  ignored. - `pnpm exec prettier --check absorb-verification-ledger.md`: passed after
  formatting the ledger-only claim and handoff edits.
- Existing harness and roadmap: API absorb behavior uses
  `but_testsupport::writable_scenario`, with
  `absorb-rejected-hunks.sh` covering the rejection-counting fixture. CLI
  behavior uses `Sandbox::init_scenario_with_target_and_default_settings` and
  the existing `two-stacks`, `absorb-parent-before-child`,
  `upstream-integrated-with-updates`, and `single-branch-mode` scenarios.
  W02 should extend `cargo test -p but --test but command::absorb` for paired
  old/new, old-only, mixed full/partial, and exact residual-content cases, with
  `cargo test -p but-api legacy::absorb::tests` as the narrow unit check for
  selector/routing helpers. W03 should extend the same fixture-backed API/CLI
  targets for late rejection, cross-branch rejection, planning-created blank
  commits, and landed-target failure; proposed test names remain unassigned.
- Design decisions or source references established: current focused coverage
  protects per-path descending application order, lock ambiguity rejection,
  deletion-boundary lock matching, rejection count units, CLI ancestor/descendant
  ordering, partial selection interpretation, dry-run preservation, and landed
  or unsupported-worktree refusal. It does not reproduce F1-F3 or establish
  atomicity. The implementation remains the reviewed PR tip, while the checkout
  includes another applied local-workspace branch.
- Remaining failures, uncertainty, or blockers: no focused baseline failures.
  Results are not an isolated checkout of the published PR because the applied
  `fix-cli-local-workspace-discovery` branch, unrelated forge/GitHub edits, and
  untracked notes are present. The existing tests do not prove paired-selector
  fidelity, stale-coordinate safety, fallback amend-hint behavior, or the
  approved atomic failure contract. No `but setup`, branch switch, cleanup, or
  release-wide validation was performed.
- Next packet and first concrete action: W01; audit planner/executor side
  effects, transaction coverage, snapshot timing, and plan/apply caller
  contracts, beginning at `absorption_plan_with_perm`, `ensure_target_commit`,
  `absorb_with_perm`, `commit_amend_only_impl`, and
  `crates/but-transaction/src/lib.rs`.

### W01 - Audit State Boundaries And Caller Contracts

- Packet / owner / status / date: W01 / GitHub Copilot / done / 2026-10-04.
- Prerequisites and approvals checked: W00 is complete with the same checkout
  `8aa707cc9c95eb2650a1ab70f8496d03bbc5229f`, absorb branch
  `64436487e7c57bdfde3da4785f1e9d6e52c38d50`, secondary applied branch
  `bcd48469bd88940245a6fadbec47801a288a3778`, and base
  `16315f27b97c8132931aa8d84b0b892d1d2a7a43`. Product policy remains approved;
  W04 technical design approval remains required before production changes.
- Source revision, applied branches, relevant dirty state: `git rev-parse`,
  `git status --short`, and `target/debug/but status` matched W00 before this
  audit. The pre-existing modified forge/GitHub files, untracked absorb notes
  and briefs, and `xen.d` remain; W01 changed only this ledger.
- Changed files and behavioral scope: source inspection and ledger evidence only;
  no production code, tests, fixtures, dependencies, SDK output, branches,
  commits, pushes, or GitHub state changed.
- Evidence: source reads covered [legacy absorb](crates/but-api/src/legacy/absorb.rs),
  [CLI orchestration](crates/but/src/command/legacy/absorb.rs),
  [amend](crates/but-api/src/commit/amend.rs), [workspace amend](crates/but-workspace/src/commit/commit_amend.rs),
  [materialization](crates/but-rebase/src/graph_rebase/materialize.rs),
  [transaction](crates/but-transaction/src/lib.rs), [selection](crates/but-core/src/tree/mod.rs),
  [hunk conversion](crates/but-hunk-assignment/src/lib.rs), and all caller
  surfaces in the table. No new executable test was run because W01 questions
  were resolved by source; W00's focused baseline remains the executable baseline.
- Observed current sequence: selected file/hunk or branch -> planner reads the
  physical HEAD worktree and reconciles assignment rows -> dependency/assignment
  routing -> blank commit insertion for an empty target -> CLI-only landed-target
  filtering and plan display -> CLI/API pre-operation snapshot -> singleton hunk
  conversion and amend -> each amend materializes objects, checkout, refs/HEAD,
  and workspace state -> rejection-count reporting and undo hint. Desktop, Lite,
  HTTP, and direct N-API can separate planning from apply.
- Staged-input and selection findings:
  - Amend reads `ChangesSource::Head`; tree creation uses physical HEAD worktree
    content and selected `DiffSpec` headers. Materialization uses a merge-base
    override to keep consumed changes from reappearing in one completed step.
  - `Transaction::amend_commit` retains an in-memory rebase and maps rewritten
    IDs, but each amend still takes a source checkout. Its override setter assigns
    one tree, so source does not prove repeated staged amendments compose consumed
    selections correctly.
  - `CommitAbsorption` has hunk vectors but no coupled-group identity or source
    blob/version. Current scheduling splits every hunk and sorts by path and
    `new_start`; conversion sets `previous_path: None`. This establishes the
    need for F1/F2 regressions, not their outcome.
  - Separate plan/apply requests carry no revision, blob identity, or
    compare-and-swap precondition. A repository lock does not exclude external
    Git or editor changes.
- Supported-domain audit:

| Input domain                                    | Observed current behavior                                                                                                         | Required disposition                                                  |
| ----------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------- |
| Full and partial text hunks                     | Old/new coordinates are normalized by `to_additive_hunks`; current tests cover ordinary cases                                     | W02 must confirm coupled and old-only cases before support is claimed |
| Coupled old/new and several old-only selections | No group identity survives singleton scheduling                                                                                   | F1/F2 remain open                                                     |
| Renames                                         | Conversion explicitly sets `previous_path: None`                                                                                  | W04 must reject/defer unless tested support is approved               |
| Binary or large files                           | Headers may be absent, dependency locks fall back, and tree code has binary/large rejection paths                                 | Unverified; classify rather than rely on fallback                     |
| Mode changes and non-UTF-8 paths                | Trees/paths use entry kinds and bytes, but assignment, lock, and UI paths use lossy decoding too                                  | Unverified; require evidence or safe rejection                        |
| Linked worktrees                                | Public CLI refuses linked-worktree changes; lower-level amend can model a linked source while legacy absorb uses HEAD             | Current public disposition is reject                                  |
| Immutable or landed targets                     | Lower-level amend errors on immutable commits; CLI filters merged-upstream targets after planning and can apply remaining entries | Current mixed skip conflicts with approved all-or-nothing policy      |
| Merge histories                                 | No absorb-specific coverage; graph guidance warns against parent-order assumptions                                                | Defer or safely reject until W04 specifies evidence                   |

- W03/W06/W07 test roadmap:

| Risk / source boundary                                      | Existing fixture/helper                                                          | Proposed scenario                                                                        | Exact observable assertion                                                                                   | Packet              |
| ----------------------------------------------------------- | -------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------ | ------------------- |
| Per-step materialization before later rejection             | `writable_scenario("absorb-rejected-hunks")` and existing rejection-count test   | First group succeeds alone; combined invocation reaches a deterministic later rejection  | Target/descendant blobs, refs/HEAD, worktree/index, metadata/assignments, and oplog equal pre-planning state | W03                 |
| Planning-created blank commit and assignment reconciliation | `writable_scenario`; CLI `Sandbox` empty-stack patterns                          | Capture state before planning; select an empty target plus blocker; repeat dry-run       | No reachable blank commit/ref, assignment change, worktree/index change, or visible oplog entry survives     | W03                 |
| Independent branches and selected landed target             | `two-stacks`, `upstream-integrated-with-updates`, existing merged-upstream tests | Pair an eligible selection with blocked cross-branch or selected landed/immutable target | No subset succeeds; both branches and selected work remain unchanged                                         | W03                 |
| Stale split plan/apply request                              | Desktop modal, Lite query/mutation, generated SDK types                          | Plan, alter worktree or target ref, submit old plan                                      | Explicit stale/blocking result and no selected publication; distinguish direct API and transport coverage    | W03/W06             |
| Coupled selector and old-only coordinates                   | `writable_scenario`, CLI `Sandbox`, `to_additive_hunks` tests                    | W02 paired line-5 replacement and several old-only selections through planner/executor   | Exact target blobs and residual worktree retain only selected content in coupled order                       | W02                 |
| Transaction composition candidate                           | `crates/but-transaction/src/tests.rs` rollback/dry-run `Sandbox` tests           | If W04 considers reuse, chain repeated `amend_commit` calls against one source           | Each staged operation consumes only intended residual selection; rollback leaves refs/HEAD unchanged         | W04 input, then W05 |
| Materialization/oplog failure                               | No absorb-specific fault injection found                                         | Use existing hook if found; otherwise identify the first publish boundary                | Never claim rollback after object, checkout, ref/HEAD, refresh, or oplog failure without evidence            | W07                 |
| User-visible outcome                                        | Desktop Playwright `absorb.spec.ts`, CLI snapshots, Lite mutation/query tests    | Surface atomic failure and stale plan through each adapter                               | No success toast/JSON/undo hint for skipped or rolled-back selected input                                    | W06                 |

- W04 design questions, not decisions:
  - Can planning become side-effect-free, or can blank-target and assignment writes
    be staged with amendment inside one atomic boundary?
  - Can `but-transaction` be reused after source-consumption composition is
    proven, and how will its project-database exclusion be handled?
  - Is stale-plan safety a source snapshot/version, immediate revalidation, or
    another approved API contract?
  - What replaces the public `usize` partial-success contract for CLI, SDK,
    Desktop, Lite, HTTP, and direct N-API callers?
  - Which domains are safely rejected first release, and what is the exact report
    for selected immutable or landed targets?
  - What is reported after checkout, ref, database, workspace-refresh, or oplog
    finalization failure when restoration is uncertain?
- Remaining failures, uncertainty, or blockers: no environment blocker, but W01
  identifies planning persistence, per-step publication, landed-target skipping,
  no stale-plan contract, project-database exclusion, unverified staged-source
  composition, and untested finalization recovery as implementation/release
  blockers. Process/disk interruption has not been audited as crash-proof ACID.
- Next packet and first concrete action: W02; add the fixture-backed paired
  old/new line-5 public-operation regression from its brief, run it immediately,
  and record exact target/residual content before any production fix.

### W02 - Reproduce Selection Failures

- Packet / owner / status / date: W02 / GitHub Copilot / done / 2026-10-04.
- Prerequisites and approvals checked: W00 and W01 were complete before the
  claim. Product-policy decisions remain fixed; W04 technical design approval
  remains required before any production implementation.
- Source revision, applied branches, relevant dirty state: checkout branch
  `gitbutler/workspace` and `HEAD`
  `8aa707cc9c95eb2650a1ab70f8496d03bbc5229f`; applied absorb branch
  `64436487e7c57bdfde3da4785f1e9d6e52c38d50`, secondary applied branch
  `bcd48469bd88940245a6fadbec47801a288a3778`, and base
  `16315f27b97c8132931aa8d84b0b892d1d2a7a43`. Pre-existing modified
  forge/GitHub files, all untracked plan/brief files, and `xen.d` were
  preserved. W02 additionally modified the absorb test module and added only
  the three scenario fixtures below.
- Changed files and behavioral scope: added fixture-backed tests in
  [legacy absorb tests](crates/but-api/src/legacy/absorb.rs),
  [paired selection fixture](crates/but-api/tests/fixtures/scenario/absorb-paired-selection.sh),
  [mixed selection fixture](crates/but-api/tests/fixtures/scenario/absorb-mixed-selection.sh),
  and [paired routing fixture](crates/but-api/tests/fixtures/scenario/absorb-paired-routing.sh).
  No production selection, routing, transaction, amendment, transport, SDK,
  dependency, branch, commit, push, or GitHub state changed.
- Local hypothesis and discriminating check: singleton scheduling loses a
  paired selection's shared coordinates or reuses old-side coordinates after a
  prior materialization. Each test compares independently constructed literal
  target content, worktree bytes, and residual hunk headers after the real
  executor; planner cases route through `AbsorptionTarget::Hunks` first.
- Evidence, case mapping, and observed outcomes:
  - S1, `paired_old_and_new_hunk_selections_preserve_their_shared_replacement`,
    uses `absorb-paired-selection`: its zero-context precondition is one
    `-1,10 +1,10` hunk. The direct plan selects `-5,1 +0,0` then
    `-0,0 +5,1`. Expected target order is `old-01` through `old-04`, `new-05`,
    then `old-06` through `old-10`; the initial target assertion observed
    `new-05`, `old-01`, `old-02`, `old-03`, then `old-05` through `old-10`.
    Worktree-byte preservation passes, but the final test fails earlier on a
    malformed residual diff: observed `-1,0 +1,4` and `-2,9 +6,5`, expected
    `-1,4 +1,4` and `-6,5 +6,5`. This confirms F1 at the executor boundary.
  - S2, `multiple_old_side_hunk_selections_do_not_use_shifted_coordinates`,
    uses a fresh `absorb-paired-selection` copy and selects original old lines
    2 and 7. Expected target lines are 1, 3, 4, 5, 6, 8, 9, and 10; observed
    target lines are 1, 3, 4, 5, 6, 7, 9, and 10. Its worktree and residual
    assertions pass before the intentional target-content failure. This confirms
    F2 independently of the paired case.
  - S3, `mixed_full_and_paired_hunk_selections_preserve_only_selected_content`,
    uses `absorb-mixed-selection`: it selects the full first `-1,4 +1,4` hunk
    plus `-15,1 +0,0` and `-0,0 +15,1` from the independent second region.
    The first region is applied, but the paired second region corrupts target
    content and residual coordinates. Final residual headers are
    `-13,0 +13,2` and `-14,9 +16,7`, expected `-13,2 +13,2` and
    `-16,7 +16,7`.
  - S4a, `planner_and_executor_preserve_paired_hunk_selection_content`, routes
    the S1 pair through `absorption_plan_with_perm(AbsorptionTarget::Hunks)` in
    the unambiguous one-target fixture. Planner assertions pass: one destination,
    the only mutable target commit, and old/new input order retained. Applying
    that plan reproduces S1 target corruption, narrowing this case to executor
    behavior rather than its single-target routing.
  - S4b, `planner_does_not_split_a_paired_selection_across_dependency_targets`,
    uses `absorb-paired-routing`, with one mutable commit changing lines 5-6 and
    a later commit changing lines 15-16. The clean test observes two plan groups
    instead of one. A temporary test-only diagnostic run, removed before final
    validation, recorded `+5,1` routed by `HunkDependency` to `change first
region` and `-5,1` routed by `DefaultStack` to `change second region`.
    This is a planner failure before executor application, not an ambiguity to
    guess through.
- Commands and final results:
  - `cargo test -p but-api legacy::absorb::tests -- --list` discovered 9 tests:
    the four W00 baseline tests plus the five W02 cases named above.
  - `cargo test -p but-api legacy::absorb::tests` completed with 4 passing
    baseline tests and 5 expected failing W02 tests:
    `paired_old_and_new_hunk_selections_preserve_their_shared_replacement`,
    `multiple_old_side_hunk_selections_do_not_use_shifted_coordinates`,
    `mixed_full_and_paired_hunk_selections_preserve_only_selected_content`,
    `planner_and_executor_preserve_paired_hunk_selection_content`, and
    `planner_does_not_split_a_paired_selection_across_dependency_targets`.
  - `cargo test -p but-core to_additive_hunks`: 7 passed, 0 failed. This does
    not clear F1/F2 because the regressions cross planning/executor boundaries.
  - `cargo fmt --check -p but-api`, `bash -n` for all three new scenario scripts,
    and `git diff --check -- crates/but-api/src/legacy/absorb.rs` passed.
- Design decisions or source references established: F1 has two distinct
  failure sites: planner splitting coupled sides when candidate routes differ,
  and executor splitting/materializing even a planner-preserved pair. F2 is a
  repeated materialization/source-coordinate failure. The existing
  `to_additive_hunks` tests pass, so W04 must evaluate grouped execution or a
  source-snapshot/composed-patch design at the absorb boundary; do not duplicate
  the normalization helper or assume its unit coverage proves application safety.
- Remaining failures, uncertainty, or blockers: the workspace intentionally
  contains the five expected failing W02 tests listed above. F3 fallback-hint
  behavior and F4 repeated-materialization cost remain untested. W02 does not
  establish atomicity, fix any defect, select a technical design, or authorize
  production changes.
- Next packet and first concrete action: W03; claim
  [the atomicity-test brief](absorb-w03-atomicity-tests.md), capture semantic
  pre-planning state in an existing writable fixture, then create the late
  rejection case before changing implementation behavior.

### W03 Execution Evidence (complete; W04 handoff)

- Packet claim: GitHub Copilot claimed W03 on 2026-10-04 after reconfirming the
  W00 source identities and the W01/W02 handoff. No production code, public API,
  generated binding, dependency, commit, branch, push, or GitHub state changed.
- Source under test: `8aa707cc9c95eb2650a1ab70f8496d03bbc5229f`, plus the applied
  absorb branch `64436487e7c57bdfde3da4785f1e9d6e52c38d50` and local-workspace
  discovery branch `bcd48469bd88940245a6fadbec47801a288a3778` recorded by W00.
- A1 boundary and fixture:
  `but_api::legacy::absorb::{absorption_plan,absorb}` over
  `writable_scenario("absorb-rejected-hunks")`. This runs planning and the
  public wrapper separately, with state captured before planning. It does not
  exercise CLI eligibility filtering; that remains A4 coverage.
- Applicable-alone control:
  `applicable_hunk_is_absorbed_through_public_planning_and_execution` selects
  line 10. It planned one target (`refs/heads/feature`), returned zero rejected
  groups, wrote `selected change` to that commit, and retained the exact
  worktree bytes. Command:
  `cargo test -p but-api applicable_hunk_is_absorbed_through_public_planning_and_execution -- --nocapture`
  passed: 1 test, 0 failed.
- Expected-red late-rejection policy test:
  `rejected_hunk_after_applicable_hunk_leaves_public_absorb_invocation_unchanged`
  selects independent full-line hunks 10 then 5 on one file. The executor's
  descending-new-line scheduler reaches the valid line-10 amendment before the
  stale line-5 rejection. Command:
  `cargo test -p but-api rejected_hunk_after_applicable_hunk_leaves_public_absorb_invocation_unchanged -- --nocapture`
  compiled and failed as an approved policy test: 0 passed, 1 failed, with
  `rejected groups: 1`.
- A1 observed semantic delta: the exact worktree bytes, file type/mode,
  porcelain status, symbolic HEAD, `main` and `origin/main` refs, project
  metadata, and sorted hunk-assignment rows were unchanged. `feature` advanced
  from `439bfa6a2089613ad0e6ec56ecf588e160c897c4` to
  `d19528dd5ab877d3fd80c0da24ab8a018f844fe0`; its `shared.txt` blob and index
  entry changed to include `selected change`; and the previously empty oplog
  advanced to `5e4a99df306fa5c3a024c0ea8e20afc0f8935a5b`. No workspace ref was
  present before or after. This confirms partial publication despite the
  returned rejection count.
- A1 oracle dimensions: verified worktree bytes/type/mode, index path/stage/mode/blob,
  tracked status, relevant refs/HEAD, feature ancestry/blob, project metadata,
  hunk assignments, and oplog head. It intentionally does not compare unrelated
  unreachable objects, cache files, mtimes, or unrelated metadata tables; W01
  did not identify them as visible absorb effects. Planning in the `Hunks`
  target path does not reconcile assignments, so the observed empty assignment
  table is a meaningful no-change check but not coverage for planner assignment
  persistence; A3/A6 remain responsible for that path.
- A2 boundary and fixture:
  `rejection_on_one_independent_branch_leaves_all_planned_branches_unchanged`
  uses the new `writable_scenario("absorb-independent-branches")`: independent
  A/B commits merged under `gitbutler/workspace`, then one selected worktree
  hunk per file. The test persists the same applied-stack metadata used by the
  existing `two-stacks` harness, invokes `absorption_plan(AbsorptionTarget::All)`,
  and proves `a.txt` is dependency-routed to A and `b.txt` to B. It deliberately
  changes only B's returned selector to a stale line-5 header, keeping the
  real planner-derived targets and a deterministic later B rejection.
- A2 expected-red result: `cargo test -p but-api
rejection_on_one_independent_branch_leaves_all_planned_branches_unchanged --
--nocapture` compiled and failed as an approved policy test: 0 passed, 1
  failed, `rejected groups: 1`. The initial run first exposed an ambiguous
  fixture without applied metadata; the test was repaired using existing
  metadata APIs and then reached the intended policy assertion.
- A2 observed semantic delta: exact `a.txt`/`b.txt` worktree bytes and modes,
  symbolic HEAD, `main`, `B`, `origin/main`, and project metadata stayed stable.
  A advanced from `abb19371281cdfc69cad33710e4a1f08c5347c46` to
  `5195d6d21ea83f81564ef478c3fc8d00abf592f8`; the workspace ref advanced from
  `177491fe46b643c4d1efee30bbec2ab1ffa53b1c` to
  `65dbc9cd3d3a537059e88399d72f8c8cb07c25b0`; and their `a.txt` blobs plus the
  index changed to `A selected change`. The residual worktree/status removed
  `a.txt` but retained `b.txt`, planner reconciliation created the two hunk
  assignment rows, and the oplog head advanced from absent to
  `f6a4a8a9c7b0e8ba54c20fd800ab1645aedd0252`. B itself did not publish. Thus a
  rejection on one independent branch still publishes the eligible subset.
- A2 oracle dimensions: verified worktree bytes/type/mode, index path/stage/mode/blob,
  tracked status, relevant A/B/workspace/base refs and HEAD, target and workspace
  blobs/ancestry, project metadata, sorted assignment rows, and oplog head.
  The same non-primary exclusions as A1 apply. This fixture covers assignment
  reconciliation but not planning-created blank commits or dry-run behavior.
- A3 boundary and fixture:
  `planning_blank_commit_before_rejection_leaves_invocation_unchanged` uses the
  independent-branches fixture, persists applied metadata for A and B, creates
  an empty `refs/heads/empty` segment above A through the public branch-create
  API, writes a new `empty.txt`, and assigns it to that branch through the
  public assignment API. `absorption_plan(AbsorptionTarget::All)` therefore
  invokes the audited `ensure_target_commit()` blank-commit path. The returned B
  hunk is then changed to a stale line-5 selector to force a later rejection.
- A3 expected-red result: `cargo test -p but-api
planning_blank_commit_before_rejection_leaves_invocation_unchanged --
--nocapture` compiled and failed as approved policy evidence: 0 passed, 1
  failed, `rejected groups: 1`. The first placement attempt resolved the empty
  ref to A's existing commit; moving the empty segment above A reached the
  intended planner-created blank commit path.
- A3 observed semantic delta: before planning, `refs/heads/empty` pointed to
  `abb19371281cdfc69cad33710e4a1f08c5347c46` and its `empty.txt` tree entry was
  absent. After the later B rejection, `refs/heads/empty` pointed to
  `03598709389007d9335f75e0c92c5cb76841e087`, with parent
  `5195d6d21ea83f81564ef478c3fc8d00abf592f8`, and its tree contained
  `new empty-branch content`. The workspace ref advanced from
  `177491fe46b643c4d1efee30bbec2ab1ffa53b1c` to
  `27810f71dc4c8b96da92402dbc148991a9eedc5e`; A also advanced to
  `5195d6d21ea83f81564ef478c3fc8d00abf592f8` because the workspace merge was
  rebuilt. The new file became tracked in the index and disappeared from the
  residual worktree diff, while B's stale `b.txt` change remained. The three
  assignment rows, including the explicit `empty.txt` branch assignment,
  remained after planning/execution. The oplog head changed from
  `1b962e9c47843056a08dec20a6e89c75b28a314b` to
  `3f382ffc8d338f57ee4e48e5e61a4c5ab7d3c570`. Workspace metadata remained
  structurally equal, but the reachable blank commit and all related state
  should have been rolled back under the approved policy.
- A3 oracle dimensions: verified relevant worktree presence/bytes/type/mode,
  index path/stage/mode/blob, status, HEAD, A/B/empty/workspace/base refs,
  target and workspace tree entries/parents, project metadata, workspace
  metadata, sorted assignments, and oplog head. Unreachable Git objects and
  cache/mtime state remain intentionally excluded as in A1/A2.
- A4 boundary and fixture:
  `absorb_selected_landed_target_blocks_other_selected_targets` exercises the
  CLI `--json absorb` caller over
  `writable_scenario("upstream-integrated-with-updates")`, with A's selected
  change landed upstream and B's selected change still eligible. The command
  output proves the filter boundary observed the landed selection through
  `skippedMergedUpstream` and retained B in the filtered plan. The test then
  requires a non-success result and unchanged A/B refs and worktree bytes.
- A4 expected-red result: `cargo test -p but --test but
absorb_selected_landed_target_blocks_other_selected_targets -- --nocapture`
  compiled and executed one test, failing as approved policy evidence. The CLI
  returned exit status 0 with JSON `ok: true`, `rejected: 0`, a plan containing
  B, and one skipped landed A commit.
- A4 observed semantic delta: A stayed at
  `756ee31783c2adf1542abe10ea254866d1464983`; B advanced from
  `536958e9343fce0fa27fd4d51f88317cca5ff78f` to
  `0d7b11c650287efbf2574df14adfe44dbce26f45`. Both worktree files retained
  their exact modified bytes, but the eligible B target was still published
  while the selected landed A target was silently filtered from execution. The
  test does not assert the full database/oplog oracle because this caller-level
  case is intentionally focused on the eligibility-filter contract; A1-A3 cover
  the broader API state dimensions.
- A5 cross-branch success control:
  `independent_branch_absorption_succeeds_for_all_planned_targets` uses the
  same independent A/B fixture and applied-stack metadata as A2, but leaves both
  planner-derived selectors valid. `cargo test -p but-api
independent_branch_absorption_succeeds_for_all_planned_targets -- --nocapture`
  passed: 1 passed, 0 failed. Both targets were planned, `absorb()` returned
  zero rejected groups, A and B each advanced, and exact `a.txt`/`b.txt`
  worktree bytes were preserved. This is the cross-branch counterpart to the
  A1 applicable-alone control; it does not establish rollback or a final oplog
  contract.
- A6 blocked dry-run control:
  `dry_run_with_selected_landed_target_keeps_state_unchanged` runs the CLI
  `absorb --dry-run` over the upstream-integrated fixture with selected landed A
  and eligible B changes. `cargo test -p but --test but
dry_run_with_selected_landed_target_keeps_state_unchanged -- --nocapture`
  passed: 1 passed, 0 failed; both branch refs and the complete JSON status
  remained unchanged. The existing `dry_run_shows_plan_without_changes` test
  covers a valid preview and workspace-ref stability. A planner-created
  blank-commit dry-run counterpart remains unimplemented and is explicitly an
  A6 gap, because the current W03 tests have not yet combined the A3 empty
  target with the CLI dry-run entry point.
- A6 planner-created-commit dry-run result:
  `dry_run_with_planner_created_blank_target_keeps_state_unchanged` uses the
  CLI `two-stacks` fixture, creates an empty branch with
  `branch new empty --above A`, writes `new.txt`, and invokes the real
  `absorb --dry-run` entry point. The test compiled and executed one test, then
  failed as approved policy evidence. The empty branch ref moved from
  `9477ae721ab521d9d0174f70e804ce3ff9f6fb56` to
  `785193e4242268ad304c181fde5527f7ded10f06`; A and B stayed unchanged. This
  proves planning materializes a reachable target before the CLI dry-run check.
  The assertion also compares the full CLI status JSON, so any worktree/index
  difference will be reported if reached; the observed failure occurs first at
  the ref invariant.
- A6 oracle dimensions: valid and blocked dry-run controls compare relevant
  refs and CLI status JSON. The planner-created case currently reaches the
  expected-red ref mutation before its status comparison; API A3 covers the
  broader worktree/index/metadata/assignment/oplog oracle for the same
  planner-created target followed by a later rejection.
- W03 handoff status: A1-A6 now have executable evidence, with A1-A4 and the
  planner-created A6 case intentionally red under the approved all-or-nothing
  policy. A5 and the ordinary/blocked dry-run controls pass. Scoped Rust and
  Markdown formatting, fixture syntax, whitespace, and test-registration checks
  pass. W04 approval was recorded from the Author on 2026-10-04.

## W05 Implementation Evidence

- Source identity: current absorb worktree, with unrelated changes preserved;
  exact revision and dirty-state classification remain as recorded above.
- Completed slice: `absorption_steps_for_application()` now preserves each
  planner-produced commit group instead of splitting it into singleton hunk
  amendments. This keeps paired old/new selectors in their original coordinate
  space and removes the obsolete descending-order scheduler contract.
- Completed slice: absorb amendments are composed through one in-memory editor,
  cumulative checkout cancellation, complete descendant mappings, and one
  materialization after all execution groups succeed. Rejected invocations
  publish neither graph changes, assignments, blank commits, nor oplog entries.
- Focused validation: `cargo test -p but-api
legacy::absorb::tests::paired_old_and_new_hunk_selections_preserve_their_shared_replacement
--no-default-features` passed. The absorb unit slice passed 9 tests and
  `rejected_hunk_after_applicable_hunk_leaves_public_absorb_invocation_unchanged`
  passed. The cross-branch atomicity test remains red because planning itself
  persists assignment rows before execution receives the plan.
- Completed slice: planning is non-persisting and brackets routing with one
  comprehensive source revision. Each serialized plan entry binds that revision
  to its route, target, selected hunks, reason, and diff-context setting. Apply
  rejects changed, caller-tampered, or unstamped plans before editor creation.
- Completed slice: deferred blank targets carry an optional full ref, verify its
  anchor, and insert the blank commit only in the staged editor. Successful
  publication reconciles residual assignments; empty residual state clears stale
  rows. Required oplog checkpoint preparation fails before mutation, and the
  checkpoint commits immediately after graph materialization before other
  fallible finalization.
- W04 scope-amendment proposal: add an optional, backward-compatible full ref
  name to serialized `CommitAbsorption` for a deferred blank target. When set,
  `commit_id` remains the stale-plan anchor for the reference, execution verifies
  it, stages `insert_blank_commit` below that reference in the same editor, and
  amends the staged blank commit. Existing plans omit the field and retain current
  behavior. This changes the N-API/SDK plan contract. The Author explicitly
  approved this proposal on 2026-10-04 for the current documented scope.
- Validation on 2026-10-04:
  `cargo test -p but-api legacy::absorb::tests:: --no-default-features` passed
  17 tests; `cargo test -p but --test but command::absorb::` passed 18 tests;
  targeted Clippy for `but-api`, `but-hunk-assignment`, `but-workspace`, and
  `but` with `--all-targets --no-deps -- -D warnings` passed; linear and graph
  SDK declarations regenerated; targeted Prettier and `git diff --check` passed.
  Generated `blankCommitRef` and `sourceSnapshotTree` are optional. A final
  independent focused audit found no remaining W05 blockers.
- W05 limitations handed to W06/W07: post-publication assignment reconciliation
  can still report a finalization error, but the undo checkpoint is already
  published. Fault injection for checkpoint/finalization failures belongs to
  W07. Caller interpretation of nonzero rejection remains W06 work.
- Commit status: ready for the authorized W05 checkpoint commit.

## Release Checklist

- [ ] Design explanation reviewed first; D1-D5 resolved or explicitly deferred.
- [ ] Confirmed defects reproduced before fixes; existing regressions retained.
- [ ] Full-diff review completed; no weakened assertions or duplicate patch engine.
- [ ] Runtime and operation-count evidence meets agreed budget.
- [ ] All affected callers and transport contracts accounted for.
- [ ] Tests, lint, formatting, and applicable CI recorded for release revision.
- [ ] Conflict behavior and remaining limitations described accurately.
- [ ] PR description distinguishes implemented guarantees from follow-up proposals.
- [ ] Only authorized files committed/pushed; unrelated changes preserved.
- [ ] Review replies and resolutions match published fixes, not plans.
