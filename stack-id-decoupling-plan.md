# Stack-ID Decoupling Plan (apps/desktop)

## Why

The backend API is moving toward identifiers that don't require a stack id:
mutations are keyed on commit ids and ref names (`RelativeTo`), and several
modes — single-branch, unmanaged workspaces — have no stack id at all.

The frontend in `apps/desktop` still threads `stackId` through every mutation
arg type and wraps every call in `ensureValue(stackId)` to satisfy TypeScript.
That makes the read-only mode (`stackController.isReadOnly`, true when stackId
is undefined) a tripwire instead of a graceful no-op — any leaked action
button throws.

## Status (2026-06-04)

**Branch:** `phase1-stack-id-cleanup` (one squashed commit on top of master).

**Done:** Phase 1 + Phase 2 in full. All by-id stack-id-keyed
`selectWorkspaceStackDetails(workspaceDetails, stackId, ...)` lookups inside
the `StackView` subtree are gone. Stack/segment/commit data now flows as
props from the top-level `head_info` query.

**Tests:** 59 passing / 1 failing / 1 skipped in the Playwright e2e suite.
The one failing test
([refInfoEdgeCases.spec.ts:22](e2e/playwright/tests/refInfoEdgeCases.spec.ts:22),
_"renders a workspace stack whose tip ref has been deleted"_) is the only
remaining baseline; it needs a backend change (see Phase 3 below). The
companion test at [line 48](e2e/playwright/tests/refInfoEdgeCases.spec.ts:48)
(_"can interact with a workspace stack that has a null id"_) now passes.

**What landed:**

- Phase 1: 10 of 19 `ensureValue(stackId)` calls removed; stack-id
  arg made optional or dropped on `updateCommitMessage`, `uncommit`,
  `squashCommits`, `tearOffBranch`, `createReference`,
  `CreateCommitRequest`, `commitDropHandler`, plus their call sites.
- Phase 2 (prop-passing): `MultiStackView → StackView` (`Stack`),
  `StackView → StackPanel/StackDetails` (`branches`, `topBranchName`),
  `StackDetails → BranchView` (`segment`, `branches`), `StackDetails`
  derives `commit` for `CommitView`. `BranchList → BranchCard →
CreateReviewBox/PushButton/BranchHeaderContextMenu`
  (`segment`, `branches`). `BranchView → BranchReview → ReviewCreation/
StackedPullRequestCard/CanPublishReviewPlugin` (`segment`,
  `branches`, or narrower types where the consumer reads less).
- `StackController` takes `stackId: () => string | undefined`.
- `isCommitView` requires only `commitId`; anonymous-segment commits
  now open the drawer.
- `branchChanges` query keys on branch name; `branchChanges`
  invalidations switched to list or branch-name granularity.
- `CommitKey.branchName` and
  `ExclusiveAction[edit-commit-message].branchName` are optional.

## Investigation summary (2026-06-04)

The 19 `ensureValue` call sites in `apps/desktop` fall into three categories
based on what the corresponding Tauri handler actually does with `stack_id`:

### Category A — Tauri handler already ignores stackId

The frontend passes `stackId` and the Tauri command silently drops it on
deserialize. **Pure frontend cleanup**, no backend coordination needed.

| Tauri command             | Real signature (besides ctx, projectId)                     | Frontend mutation          |
| ------------------------- | ----------------------------------------------------------- | -------------------------- |
| `commit_reword`           | `(commit_id, message, dry_run)`                             | `updateCommitMessage`      |
| `commit_amend`            | `(commit_id, changes, dry_run)`                             | `amendCommit`              |
| `commit_squash`           | `(subject_commit_ids, target_commit_id, strategy, dry_run)` | `squashCommits`            |
| `commit_uncommit`         | `(subject_commit_ids, assign_to: Option<StackId>, dry_run)` | `uncommit`                 |
| `commit_uncommit_changes` | `(commit_id, changes, assign_to: Option<StackId>, dry_run)` | (used by stackDropHandler) |
| `commit_create`           | `(relative_to, side, changes, message, dry_run)`            | `commitCreate`             |
| `commit_move`             | `(subject_commit_ids, relative_to, side, dry_run)`          | `commitMove`               |
| `commit_insert_blank`     | `(relative_to, side, dry_run)`                              | `insertBlankCommit`        |
| `move_branch`             | `(subject_branch, target_branch, dry_run)`                  | `moveBranch`               |
| `tear_off_branch`         | `(subject_branch, dry_run)`                                 | `tearOffBranch`            |
| `create_reference`        | `(request)` — returns `Option<StackId>`                     | `createReference`          |

The `assign_to: Option<StackId>` in the uncommit variants is the only
remaining stack-id surface here. Passing `None` is the correct
single-branch/unmanaged behavior — surfaced hunks land in the worktree
unassigned ([commit/uncommit.rs:169-202](crates/but-api/src/commit/uncommit.rs:169)).

### Category B — dead arg on the backend

Tauri command declares `stack_id` but the handler is `let _ = stack_id;`.
One-line backend trim plus the frontend cleanup.

| Tauri command   | Location of dead arg                                                             |
| --------------- | -------------------------------------------------------------------------------- |
| `remove_branch` | [crates/but-api/src/legacy/stack.rs:265](crates/but-api/src/legacy/stack.rs:265) |

### Category C — Tauri handler genuinely uses stackId

Passes `stack_id` through to the `gitbutler_branch_actions` layer where the
operation is keyed on it. Refactoring these requires a backend change to
expose a ref-name-based entry point.

| Tauri command                 | What stackId is used for                                                                                         |
| ----------------------------- | ---------------------------------------------------------------------------------------------------------------- |
| `create_branch`               | Locate parent stack to anchor the new branch ([legacy/stack.rs:165,177](crates/but-api/src/legacy/stack.rs:165)) |
| `update_branch_name`          | Passed through to action                                                                                         |
| `update_branch_pr_number`     | Passed through to action                                                                                         |
| `integrate_branch_with_steps` | Passed through to action                                                                                         |
| `integrate_upstream_commits`  | Passed through to action                                                                                         |
| `push_stack`                  | Inherently stack-scoped                                                                                          |
| `unapply_stack`               | Inherently stack-scoped                                                                                          |
| `update_stack_order`          | Per-item `Option<StackId>`; already half-handles unmanaged                                                       |

### Special case — `stackDropHandler.ts`

Of its 5 `ensureValue(stack.id)` calls:

- **3 are dead args** that the mutation drops:
  - Line 98 → `moveChangesBetweenCommits` (param dropped per [stackService.svelte.ts:582-588](apps/desktop/src/lib/stacks/stackService.svelte.ts:582))
  - Line 154 → same as above
  - Line 222 → `commitMove` (uses `relativeTo.targetBranchName`, not the id)
- **2 are load-bearing**:
  - Line 125 → `diffService.assignHunk()` — assignment target routes to a stack
  - Line 198 → same

The assignHunk calls are Phase 2 work — mirror the `assign_to: Option<StackId>`
pattern in `HunkAssignmentTarget` so unmanaged mode can pass `None`.

## Plan

### Phase 1 — Frontend dead-arg cleanup ✅ DONE (commit `56ae70e`)

10 of 19 `ensureValue(stackId)` calls removed by making the corresponding
mutation arg types optional or removing `stackId` entirely. The Tauri
backend was already ignoring it on deserialize.

- `stackEndpoints.ts`: optional/dropped on `updateCommitMessage`,
  `uncommit`, `squashCommits`, `tearOffBranch`, `createReference`,
  `CreateCommitRequest`. Cache invalidations conditional.
- Call sites in `CommitView`, `CommitContextMenu`, `BranchCommitList`,
  `NewCommitView`, `macros.ts`, `commitDropHandler.ts`,
  `stackService.svelte.ts` all updated.
- Remaining 9 `ensureValue` calls either guard post-`newStackMutation`
  results (real defensive code given SDK type-widening) or feed
  Category C mutations that still need stackId on the backend.

### Phase 2 — Push stack data as props ✅ DONE (commits `3f5dd9c` … `83e4ad6`)

The big architectural shift. New data flow (now landed):

```
WorkspaceView
  → stackService.stacks(projectId)        // hits head_info (workspaceDetails)
  → MultiStackView {stacks}
    → {#each stacks as stack}
      → StackView {stack}                 // ✅ full Stack object
        → StackController({stack})        // ✅ controller takes Stack
        → StackPanel {branches, topBranchName, ...}
        → StackDetails {branches, ...}
          → BranchView {segment, branches, ...}
            → BranchReview {segment, branches, ...}
              → CanPublishReviewPlugin {segment, ...}
              → ReviewCreation {segment, branches, ...}
              → StackedPullRequestCard {segment, branches, ...}
          → CommitView {commit, ...}      // commit derived in StackDetails
        → BranchList → BranchCard {segment, branches, ...}
          → CreateReviewBox {segment, branches, ...}
          → PushButton {segment, branches, ...}
          → BranchHeaderContextMenu {contextData.branch = segment}
          → BranchCommitList {segment}    // already prop-based pre-refactor
```

Every per-component re-query through
`selectWorkspaceStackDetails(workspaceDetails, stackId, ...)` inside the
`StackView` subtree is gone. The two related side-effects:

- The `branchChanges` query, which was the only stack-id-keyed cache tag
  inside this tree, was also a backend dead-arg. Slice 6 keys it on branch
  name instead.
- `StackController.isCommitView` no longer requires `branchName` (Slice 4),
  so clicking a commit in an anonymous segment now opens the drawer.

### Phase 2.x — out-of-tree by-id components (not done, lower priority)

Same prop-passing pattern, but in separate flows that weren't on the
critical path. Each is a self-contained follow-up:

- [`BranchesViewBranch.svelte`](apps/desktop/src/components/views/BranchesViewBranch.svelte) /
  [`BranchesView.svelte`](apps/desktop/src/components/views/BranchesView.svelte) —
  branches page; uses `branchDetails` + `commitChanges`.
- [`BranchIntegrationModal.svelte`](apps/desktop/src/components/branch/BranchIntegrationModal.svelte) —
  uses `commitsByIds` + `commitById` keyed on stackId. Modal-only flow.
- [`CommitFailedFileEntry.svelte`](apps/desktop/src/components/commit/CommitFailedFileEntry.svelte:82) —
  uses `branches` with `lock.target.subject` as stackId. Different shape;
  may need its own treatment.
- [`EditCommitPanel.svelte`](apps/desktop/src/components/workspace/EditCommitPanel.svelte),
  [`UnappliedCommitView.svelte`](apps/desktop/src/components/commit/UnappliedCommitView.svelte),
  [`AutoCommitModalContent.svelte`](apps/desktop/src/components/commit/AutoCommitModalContent.svelte) —
  use `commitDetails` (commit-id keyed, not stack-id keyed). Safe as-is.

The unused `stackService.*` methods used to be called by the StackView
tree but are now only kept for these out-of-tree consumers:
`branches`, `branchDetails`, `defaultBranch`, `commitById`,
`commitsByIds`, `branchParentByName`, `branchChildByName`,
`commits`, `unstackedCommits`, `fetchUnstackedCommits`. When all
out-of-tree consumers move to props, those methods can go too.

### Phase 3 — Backend coordinated trims (small backend PRs)

These were originally Phase 2 in this plan. Pulled out as a separate
track because the frontend prop-passing work above is what unblocks
the user-facing null-id behavior; the backend trims are quality-of-life.

1. **`remove_branch`** — delete the `stack_id` arg from
   [crates/but-api/src/legacy/stack.rs:250](crates/but-api/src/legacy/stack.rs:250).
   The body already has `let _ = stack_id;`. Frontend follow-up to drop it
   from `removeBranch` mutation args.
2. **Hunk assignment** — introduce `Option<StackId>` for the assignment target
   so `stackDropHandler.ts` lines 125 and 198 can pass `None` in unmanaged mode.
   Mirrors the `assign_to: Option<StackId>` pattern in `commit_uncommit`.

### Phase 4 — Real refactor (backend work, colleagues)

Each item needs `gitbutler_branch_actions` to grow a ref-name-based entry
point alongside its stack-id-based one. The Tauri commands then accept either
shape (or migrate to the ref-name variant outright).

1. **`update_branch_name`, `update_branch_pr_number`, `integrate_branch_with_steps`,
   `integrate_upstream_commits`** — currently key on `(stack_id, branch_name)`.
   A ref-name-only variant works if branch names are workspace-unique (open
   question, see below).
2. **`create_branch`** — currently uses `stack_id` as anchor. In unmanaged
   mode this likely means "create the first managed stack from the
   checked-out ref" — needs design.
3. **`push_stack`, `unapply_stack`** — inherently stack-scoped. UI should
   gate on `!stackController.isReadOnly` rather than refactor.
4. **`update_stack_order`** — already per-item `Option<StackId>`; UI just
   needs to filter out unmanaged stacks.

### Open design questions (for Phase 4 planning)

1. **Branch name uniqueness across stacks** — if two stacks have a branch with
   the same name, ref-name-based mutations need a disambiguation strategy.
   Likely answer: branch names are workspace-unique by construction, but
   worth confirming with the workspace metadata team.
2. **Unmanaged-mode `create_branch` semantics** — does creating a new branch
   in an unmanaged workspace bootstrap the first managed stack, or stay
   unmanaged? Either way, the API needs a way to express "anchor relative to
   HEAD or the checked-out ref" without a stack id. `RelativeTo::Reference`
   covers this on the placement side; need an equivalent for branch creation.
3. **`hunk_assignment` semantics in unmanaged mode** — confirm that
   `Option<StackId>` for assignment routing means "leave in worktree
   unassigned" when None, matching the `commit_uncommit` pattern.

## References

- Investigation transcript: chat session 2026-06-03 to 2026-06-04
- Frontend mutation layer: [apps/desktop/src/lib/stacks/stackEndpoints.ts](apps/desktop/src/lib/stacks/stackEndpoints.ts)
- Read-only mode plumbing: [stackController.svelte.ts:103-105](apps/desktop/src/lib/stacks/stackController.svelte.ts:103)
- `StackEntry` / `StackEntryNoOpt` types: [crates/but-workspace/src/legacy/ui.rs:51-180](crates/but-workspace/src/legacy/ui.rs:51)
- Tauri command registry: [crates/gitbutler-tauri/src/main.rs:506-514](crates/gitbutler-tauri/src/main.rs:506)
