# W01: Audit State Boundaries And Caller Contracts

Status: brief prepared; execution requires completed W00 evidence.
Scope: source-backed audit and test/design handoff, not implementation.

## Assignment For The Agent

Complete W01 only. Produce the side-effect and caller tables in
[the ledger](absorb-verification-ledger.md), with a concrete W03 test roadmap and
questions for W04 design review. Do not implement fixes, tests, or a transaction
wrapper. Preparing this brief does not mean that W00 or W01 has been executed.

Read current root and applicable ancestor/nested instructions, required skills,
the plan's Approved Product Policy and Execution Protocol, and the ledger's W00
evidence. Read [the graph guidance](crates/WORKSPACE_MODEL.md) for relationship
and mutation questions. Follow scoped frontend instructions when tracing callers;
this is a data-contract audit, not permission to redesign their UI.

The approved default is atomic from planning through publication for all selected
changes. Ambiguity and ineligible selected changes fail the invocation. Partial
mode and interactive resolution are deferred. Do not reopen those policies or
mistake their approval for approval of a particular implementation.

## Permissions And Boundaries

- Manual edits: plan and ledger only. Preserve unrelated work and earlier evidence.
- Read source and existing tests; run a narrowly relevant existing test only when
  it answers a concrete audit question. No new fixtures, snapshots, dependencies,
  generated SDK files, production changes, commits, pushes, or GitHub updates.
- Never exercise absorb against this development repository. Use existing
  fixture-backed tests for behavioral evidence.
- Do not create a parallel transaction framework or propose broad rewrites before
  establishing the actual gaps in existing APIs.
- The ledger owns task state and results. Keep this brief as instructions rather
  than duplicating completion checkboxes or evidence here.

## Step 1: Check Prerequisites And Claim

Verify W00 is done with usable source identity, baseline results, and test homes.
If it is missing or blocked, stop and name the prerequisite; do not silently
execute W00 under this assignment. Check for another active owner before claiming
W01 in the ledger.

Compare current source identity and relevant dirty state with W00 using the
required version-control workflow. Changed source does not require repeating all
baseline work: identify affected evidence and refresh only what is necessary.
Stop if attribution cannot be established without unauthorized workspace changes.

## Step 2: Trace The Actual Operation Boundaries

Begin at these verified-or-to-be-reverified anchors and follow deciding code,
not every forwarding layer. If renamed, record the current location.

- [Planner and executor](crates/but-api/src/legacy/absorb.rs):
  `absorption_plan_with_perm`, `ensure_target_commit`, `absorb_with_perm`, `absorb`.
- [CLI orchestration](crates/but/src/command/legacy/absorb.rs): planning,
  dry-run return, landed-target filtering, snapshot timing, and outcome reporting.
- [Amend API](crates/but-api/src/commit/amend.rs): `commit_amend_only_impl`.
- [Lower-level amendment](crates/but-workspace/src/commit/commit_amend.rs).
- [Workspace publication](crates/but-api/src/workspace_state.rs):
  `from_successful_rebase` and its materialization callees.
- [Transaction implementation](crates/but-transaction/src/lib.rs):
  `with_transaction_with_perm`, callback outcomes, staged state and finalization.
- [Selector interpretation](crates/but-core/src/tree/mod.rs) and
  [mapping/conversion](crates/but-hunk-assignment/src/lib.rs).

Write a short observed sequence in the ledger: selection -> planning -> target
creation/eligibility -> amendment -> rebase -> publication -> reporting/undo.
Annotate actual persistence points, lock ownership, and separate request boundaries.
Distinguish current behavior from the desired sequence; do not assert planning is
read-only merely because its API is called a plan.

## Step 3: Fill The Side-Effect Table

For each state category, record the responsible symbol with a source link, what
changes, when it persists, how failure affects it, and the test needed. Explicitly
mark not-applicable with evidence rather than omitting a category.

| State                | Questions the audit must answer                                                                                            |
| -------------------- | -------------------------------------------------------------------------------------------------------------------------- |
| Refs and HEAD        | Can planning create refs/commits? When are replacements published? Are expected old ref values checked?                    |
| Index and worktree   | Which calls write/check out? What happens on conflict or failure partway through publication?                              |
| Metadata             | Which updates are staged versus written immediately? What restores them?                                                   |
| Project database     | Which records are written before commit? The transaction's documented exclusion requires inspection, not assumed coverage. |
| Objects              | Are writes in memory or persisted? Distinguish unreachable objects from published history.                                 |
| Oplog/undo           | When is a snapshot taken and recorded? Can its failure occur after history publication?                                    |
| Caches/notifications | Are observers told about temporary or failed state? What invalidates cached workspace data?                                |

For error handling, separate rejection before publication, ordinary errors during
finalization, errors after history publication, and process/disk interruption.
Identify whether restoration exists and can itself fail. A returned `Err` is not
proof of unchanged state. Record uncertain-state reporting requirements instead
of promising rollback that the inspected implementation cannot establish.

Stop tracing once the owning writer/recovery boundary and discriminating test are
identified. Do not audit unrelated database tables or the entire checkout engine.

## Step 4: Check Staged Input And Selection Semantics

Answer these with implementation and existing-test references:

1. Which blobs, tree, index, or physical worktree supply each amendment's diff?
2. After one staged amendment, does the next step see its changes as consumed,
   or can it read the original physical state and apply them again?
3. What identifies a coupled selection group, and which coordinate space does
   each selector use? Does routing happen before that meaning is preserved?
4. How are rewritten target IDs mapped across successive operations?
5. Can selection normalization be reused without exposing a second independent
   implementation of patch semantics?
6. What protects a plan between separate plan/apply requests from source changes?
   Internal locks do not automatically exclude edits from external Git or editors.

Record candidate reuse points and missing capabilities. Do not conclude that an
in-memory repository or bottom-up ordering alone makes composition correct.
Where runtime evidence is needed, specify the minimal fixture and exact expected
content for W02/W03; do not invent the observation or add the test in W01.

## Step 5: Inventory Caller Contracts And Supported Inputs

Use symbol references and targeted text searches for absorb API names, command
registrations, and generated bindings. Search actual transport names too, since
dynamic invocations may not appear as Rust references. Record search scope when
claiming a surface has no caller.

Account for CLI/TUI, desktop, Lite, web if applicable, N-API, SDK, and direct API
callers. For each actual path record:

- Whether planning and applying are one call or separate requests.
- Input selections, eligibility filtering, and stale-plan assumptions.
- Current success/rejection/error shape and how users or scripts interpret it.
- What must change for atomic failure, or source-backed unaffected rationale.
- A concrete existing test home or a named coverage gap for W06.

Differentiate a shared handler from multiple transport registrations; do not
inflate the inventory with duplicate wrappers. Changing return semantics matters
even if the Rust signature remains the same. Generated declarations alone do not
prove application behavior.

Classify full/partial hunks, renamed/binary files, mode changes, non-UTF-8 paths,
linked worktrees, immutable/landed commits, and merge histories. For each, state
observed support/rejection, evidence, and proposed first-release disposition.
Do not silently expand or narrow scope; take policy-impacting proposals to W04.

## Step 6: Turn Audit Gaps Into Test And Design Inputs

Populate a compact roadmap in the ledger using these columns:

| Risk / source boundary  | Existing fixture/helper | Proposed scenario              | Exact observable assertion               | Packet          |
| ----------------------- | ----------------------- | ------------------------------ | ---------------------------------------- | --------------- |
| Fill from investigation | Verify actual API       | Mark proposed cases explicitly | Before/after state, not just exit status | W02/W03/W06/W07 |

At minimum cover late rejection after an earlier staged success, planning-created
commits, cross-branch failure, selected ineligible targets, database writes if
present, stale plan inputs, and publication/oplog failure reporting. Include
success and dry-run counterparts. Use the existing harness and precise state
comparisons rather than whole-directory snapshots containing volatile data.

For W04, list candidate atomic boundaries and their demonstrated gaps, return/error
compatibility choices, and any necessary new mechanism requiring approval. Do not
select a final architecture before W02/W03 supply their failing evidence.

## Step 7: Validate And Hand Off

Check that every audit conclusion has a source reference, existing test result,
or explicit unverified label. For any executed test, record command, source state,
nonzero test count, outcome, and what it does and does not establish. Do not rerun
all W00 tests merely to make the audit look validated.

Use the ledger's packet handoff template. W01 is complete when it contains:

- Observed operation sequence and source identity.
- Filled side-effect and caller tables, including unverified boundaries.
- Staged-input/selection findings and supported-domain classification.
- Actionable test roadmap and W04 design questions.
- Named safety gaps that block implementation/release, even if the audit is done.

An identified gap can be a successful audit result. Mark W01 blocked only when
missing access, attribution, or another prerequisite prevents the required audit;
do not spend unbounded effort proving future design correctness here.

Validate formatting of only the modified documents with existing tooling. Mark
W01 done with evidence, clear ownership, and advance Resume Here to the next
pending ready packet, normally W02 (or W03 if W02 is already complete). Do not
advance to W05 or mark technical decisions approved. Stop after W01.

## Prompt To Assign This Packet

> Execute absorb-w01-state-audit.md only after verifying W00 evidence. Audit
> persistence, transaction coverage, staged diff inputs, and actual caller
> contracts. Record source-backed tables, unknowns, and a concrete test roadmap
> in absorb-verification-ledger.md. Do not implement code or tests, commit, push,
> switch branches, or update GitHub. Stop with an audit handoff or precise blocker.
