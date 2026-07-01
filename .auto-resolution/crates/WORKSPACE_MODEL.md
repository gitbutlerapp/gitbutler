# Graph and Workspace State Guidance

Use this when changing, reviewing, or investigating Rust code under `crates/` that derives GitButler graph, workspace, branch, stack, commit, push, or rebase relationships, reachability, dependencies, ordering, operation targets, or Git graph/history/ref-placement changes.

This document captures intended direction. Prefer nearby code patterns for small legacy fixes, but avoid expanding legacy abstractions when adding new behavior.

## Core direction

GitButler is moving away from **stacks** as a primary internal abstraction.

Stacks are useful for users and UI, but Git itself has commits, refs, parent edges, and reachable subgraphs. New logic should generally model behavior in terms of Git-representable concepts and graph relationships:

- commits
- refs
- graph relationships
- dependencies between refs/commits
- target ref / target commit as frame-of-reference metadata

When an operation needs a handle inside a live graph editor, use operation-local selectors. Do not treat selectors as durable Git objects; they point into one editor revision and are derived from commits or refs inside that editor.

UI code may still present stacks, lanes, or buckets. Core read/query paths and mutations should avoid depending on those presentation buckets as source of truth.

## Preferred layering

This applies both when **changing** state and when **asking questions** about stacks, branches, commits, or dependencies. The compressed/projection views are convenient, but they are not the best internal source of truth for graph-shaped questions.


| Task | Prefer | Avoid |
|---|---|---|
| Rewrite, move, drop, insert commits or ref placements | graph editor (`but_rebase::graph_rebase::Editor`) where an editor model exists | old linear rebase engine (`but_rebase::Rebase`) |
| Ask graph/dependency/state questions about stacks, branches, commits, or ordering | `but_graph::Graph` | workspace projection / refinfo as source of truth |
| Render UI or caller state | workspace projection (`but_graph::Workspace`) and refinfo (`but_workspace::RefInfo`) | raw mutation structures directly |
| API operation targets | commit IDs and refs at the boundary; selectors inside a live editor operation | stack IDs unless legacy boundary requires it |
| Lower-level operations | explicit repo/db/meta/workspace handles | taking full `but_ctx::Context` |

If code is shaping data for an existing UI/API contract, projection/refinfo is appropriate. If code decides what may be mutated, pushed, rebased, hidden, ordered, or considered dependent/reachable, derive that from `but_graph::Graph` or the graph editor first and convert afterward. Existing workspace-shaped legacy boundaries may still use projection; avoid expanding that shape into new core logic.

## Context boundaries

`but_ctx::Context` is a broad provider for repository, database, cache, and workspace access. It is useful near API/composition boundaries, but lower-level logic should receive narrower dependencies.

Prefer acquiring permissions/handles once at the boundary, then passing explicit dependencies inward:

- repo access
- workspace/projection, only for presentation/compatibility helpers or existing workspace-shaped boundaries
- metadata (`but_core::RefMetadata`)
- database handles
- graph or graph editor state for relationship/mutation decisions
- permission-taking helper variants such as `_with_perm`

Avoid lower-level helpers reacquiring `Context` or repository write guards while another guard is held. Hidden reacquisition makes lock ordering unclear and can cause deadlocks.

## Metadata and target refs

Workspace metadata is exposed through `but_core::RefMetadata` and `but_core::ref_metadata::Workspace`.

Important concepts:

- target ref — usually something like `origin/main`; the branch/ref used as the workflow frame of reference.
- target commit ID — a commit reachable from the target ref that should stay included in the workspace context, making the workspace stable even when the target ref moves.
- workspace stacks — legacy/presentation metadata; useful for compatibility but not ideal as new operation targets.

Treat target ref and target commit as **frame-of-reference metadata**, not automatically as mutation commands.

Use target ref/commit differently depending on the operation:

| Situation | Role |
|---|---|
| Workspace/query traversal | target commit/ref can bound or extend the traversal context |
| Display/caller state | target ref names the integration frame |
| Rebase/push/action target | the selected commit/ref is the operation target, unless the command explicitly updates workspace target metadata |

Do not conflate:

- local branch `foo` vs remote-tracking branch `origin/foo`
- local branch `foo` relative to target ref `origin/main`

Those answer different questions. This matters especially for normal Git / single-branch workflows.

## `but_graph::Graph`

`but_graph::Graph` is the current best graph-shaped model of repository/workspace state. Prefer starting from it when internal code needs to understand relationships between stacks, branches, commits, refs, or dependencies.

Use it for state/query questions such as:

- which commits/refs are in scope below this head?
- what needs to be pushed before or with this ref?
- where should traversal stop relative to the target commit?
- what subgraph is relevant for this operation?
- which commits belong under a ref or stack-like UI grouping?
- what branch/ref relationships exist before deciding what to display or mutate?

Caveats:

- The graph is segment/bucket based because older UI concerns influenced it.
- It can encode ordering information Git itself does not represent, especially around refs.
- Merge parent order may not always be reliable; be careful with first-parent traversal or UI that assumes the first parent is the mainline.

## Workspace projection and refinfo

`but_graph::Workspace` and `but_workspace::RefInfo` are derived, interpreted, compressed views. They are useful for presentation, compatibility, and existing workspace-shaped boundaries, but they are lossy. The warning is broader than mutations: do not use these views as the main internal source when asking substantive new questions about stacks, branches, commits, ordering, dependencies, or reachability.

Use them for:

- frontend/caller state
- rendering
- compatibility with existing APIs

Avoid using them as source of truth for:

- Git graph/history/ref-placement mutations
- push dependency ordering
- rebase decisions
- graph topology decisions
- new internal stack/branch/commit membership questions
- reachability or dependency analysis

If code needs graph topology or accurate relationships, go back to `but_graph::Graph` or the graph editor before converting into projection/refinfo.

## Rebase and mutations

Treat the old rebase engine (`but_rebase::Rebase`) as legacy. It is linear: base plus ordered rebase steps. That assumes a meaningful contiguous range, which breaks down for graph-shaped histories, merge commits, normal Git branches, and single-branch mode.

Use the graph editor (`but_rebase::graph_rebase::Editor`) for new Git graph/history/ref-placement mutation logic where an editor model exists. Use existing metadata, database, worktree, checkout, hunk-assignment, and API bookkeeping APIs for non-graph state changes.

Important graph editor concepts:

- pick step (`but_rebase::graph_rebase::Pick`) — materialize a commit.
- reference step (`but_rebase::graph_rebase::Step::Reference`) — place or move a ref.
- none step (`but_rebase::graph_rebase::Step::None`) — placeholder after removing a pick/ref.
- `Editor::rebase()` — materializes the edited graph back into Git objects and ref edits.

The graph editor is not merely “a rebase command.” It is the in-memory graph mutation layer for history and ref-placement rewrites. It is currently created from a mutable workspace projection, so projection may be involved in editor setup even when the mutation decision should be graph-shaped.

## Push and upstream integration

Push should be graph-based, not stack-based.

Current legacy push code still has stack-shaped boundaries, such as `gitbutler_branch_actions::push_stack`. Avoid copying that shape into new code unless maintaining the legacy boundary.

For new push/dependency logic, ask graph questions:

- given this selected ref, which dependent refs/commits are below it?
- which refs must be pushed first or together?
- where does traversal stop relative to target commit/base?

Push is not itself a mutation, so using the graph editor only for ordering can be conceptually awkward. Prefer `but_graph::Graph` for dependency and ordering questions.

Upstream integration can be a useful conceptual pattern: reason over related subgraphs under workspace heads, even if existing code still uses stack terminology.

## Single branch / normal Git mode

“Single branch mode” is a product/workflow concept, not one clean Rust type.

When touching code that should work in normal Git mode:

- do not assume `gitbutler/workspace` is checked out;
- do not assume stack metadata is the operation source of truth;
- do not assume target ref equals the current branch’s remote-tracking branch;
- separate branch remote divergence from target-ref/base divergence;
- avoid managed-workspace-only assumptions unless the API explicitly requires managed workspace mode.

## Checklist before adding graph/workspace code

Ask this for both read/query code and mutation code:

1. What are the real Git objects involved: commits, refs, or both?
2. Am I asking a relationship/reachability/dependency question? If yes, why not start from `but_graph::Graph`?
3. Is this a Git graph/history/ref-placement mutation? If yes, why not use `but_rebase::graph_rebase::Editor` where an editor model exists?
4. Am I using stack IDs because the domain requires it, or because legacy APIs expose them?
5. Am I using workspace projection/refinfo only for display/compatibility, or accidentally as source of truth?
6. Does target ref/commit matter as traversal context, display frame, operation target, or workspace target metadata?
7. Could this run outside managed workspace mode?
8. Am I passing too much `but_ctx::Context` into lower layers?
9. Does this operation need immediate materialization, or can it compose graph edits first?

## Encouraged patterns

- Accept commit/ref targets at API boundaries and convert to editor-local selectors inside the operation; translate UI stack/lane selections at the boundary.
- Use `but_graph::Graph` for dependency, reachability, membership, ordering, and topology questions.
- Use `but_rebase::graph_rebase::Editor` for Git graph/history/ref-placement rewrites where an editor model exists.
- Materialize once when appropriate.
- Keep target ref/commit as lightweight frame metadata where possible.
- Convert graph state into projection/refinfo near presentation boundaries.
- Pass explicit handles instead of full context.

## Discouraged patterns

- New API accepts only `StackId` when commit/ref targets would do.
- Editor-local selectors are stored or compared outside the live editor operation that produced them.
- Operation flattens a stack and assumes a linear range.
- New push/upstream or internal query logic depends on workspace projection buckets instead of graph topology.
- Lower-level crate takes `but_ctx::Context` and reacquires locks.
- Code assumes target ref equals current branch’s remote-tracking branch.
- Code assumes a user/CLI-provided commit ID is a valid operation target merely because the object exists.

## Examples / starting points

- Graph construction and workspace projection: `crates/but-graph/tests/graph/init/with_workspace.rs`, especially `workspace_with_stack_and_local_target()` and `workspace_projection_with_advanced_stack_tip()`, shows `Graph::from_head()`, `validated()`, `into_workspace()`, and snapshot-backed graph/projection expectations.
- Target ref and target commit semantics: `crates/but-graph/tests/graph/workspace/resolved_target_commit_id.rs`, especially `prefers_target_commit_over_target_ref()` and `returns_none_with_only_extra_target()`, shows cases where target commit metadata, target refs, and extra traversal targets intentionally differ.
- Graph editor mutation patterns: `crates/but-rebase/tests/rebase/graph_rebase/replace.rs` and `crates/but-rebase/tests/rebase/graph_rebase/insert.rs` show selecting commits, replacing/inserting steps, checking `overlayed_graph()`, and materializing once.
- Workspace mutation call sites layered over the graph editor: `crates/but-workspace/tests/workspace/commit/move_commit.rs` shows creating an editor, calling `but_workspace::commit::move_commit`, materializing, refreshing workspace state, and asserting ref movement.
- Normal Git / single-branch presentation behavior: `crates/but-workspace/tests/workspace/ref_info/mod.rs`, especially `single_branch()` and `single_branch_multiple_segments()`, shows unmanaged/non-workspace `RefInfo` behavior and legacy stack compatibility expectations.
