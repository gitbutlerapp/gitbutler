# GB-1727: Avoiding duplicate Lite workspace refreshes

## Outcome and implementation

Do not try to correlate a filesystem watcher event with the mutation that caused it. The watcher batches paths after a debounce window, external Git activity can overlap a mutation, and causal identity is lost before the event reaches Lite.

Instead, compare the state that determines the workspace projection. Rust now computes an opaque, versioned checksum called `WorkspaceRevision` and attaches it to Lite's `head_info_snapshot` responses, materialized workspace mutation responses, and settled Git/workspace watcher events. The shared `head_info` API keeps returning `RefInfo` for desktop and other callers. Lite skips `head_info` invalidation only when the event revision exactly matches the revision already stored with its cached mutation/query result. A missing, failed, or different revision always refreshes.

The wire value is a string:

```text
workspace-v1:<64 lowercase SHA-256 hex characters>
```

This is an optimization hint, not an ordering token. It says “these graph inputs are equal,” not “this event happened before that response.”

## Why mutation IDs and timestamps are insufficient

- Watcher batches can contain files written by a mutation and an external process.
- One mutation can produce several filesystem changes, while several causes can collapse into one watcher batch.
- Timestamps do not describe the state that was observed and introduce clock/ordering races.
- Suppressing every event during a mutation can hide a real external change.
- Git file paths, mtimes, and storage layout are implementation details: `packed-refs` can change without changing refs, while a semantic ref change is what affects the graph.

State equality avoids those ambiguities. If an external change matters, it changes an input and therefore the checksum. If it is irrelevant to the projection, avoiding a refresh is desirable.

## Inputs to `WorkspaceRevision`

The checksum must use the same semantic inputs as workspace graph traversal and projection, encoded canonically in sorted order.

### Git state

- Resolved `HEAD`: symbolic target or detached state, plus its peeled object ID.
- Relevant refs and targets under `refs/heads/*` and `refs/remotes/*`, including workspace refs.
- Local-branch remote-tracking mappings used by graph discovery.
- Shallow-clone boundary commits.
- Linked-worktree identity, archived state, checked-out ref (or detached state), and peeled `HEAD`, when worktree support is enabled.

Do not checksum the object database, commit-graph, reflogs, lock files, mtimes, or the bytes/layout of `packed-refs`.

### GitButler metadata

- Project metadata: target ref, target commit, and push remote.
- Branch stack/order metadata.
- Workspace/virtual-branch metadata used by graph projection.
- Worktree archived state.
- The reduced forge association actually projected into `head_info`: pushed/source branch to preferred PR number.

Gerrit metadata is intentionally excluded, and both direct and mutation workspace projections ignore it.

The canonical encoder must be byte-preserving for Git names, length-prefix fields, distinguish missing values from empty values, preserve meaningful list order, and sort unordered collections. The `workspace-v1` domain/version must be part of the hashed input as well as the output prefix.

## Contract, cache, and flow

The smallest useful contract is conceptually:

```ts
type HeadInfoResponse = {
	headInfo: RefInfo;
	workspaceRevision: string | null;
};
```

Materialized workspace mutation responses get the same nullable field. Preview and dry-run responses deliberately return `null`. Workspace watcher events get a revision computed after the debounced batch is handled.

Lite does not need a second response cache. TanStack Query already owns this state. The `headInfo` query key now stores the complete `HeadInfoResponse`, while its existing consumers use a `select` function and continue to receive only `RefInfo`. When a mutation returns a workspace, `syncCoreCaches` atomically writes both its projection and revision into that same query entry. This avoids divergent cache lifetimes and means the watcher comparison always examines the revision belonging to the projection currently rendered.

Lite's decision is deliberately one comparison:

```ts
if (event.workspaceRevision !== null && event.workspaceRevision === cached.workspaceRevision) {
	// The cached projection already represents this graph-input state.
	skipHeadInfoRefresh();
} else {
	refreshHeadInfo();
}
```

The revision should be calculated from one coherent read boundary wherever possible. If computation fails or coherence is uncertain, return `null`; correctness falls back to today's invalidation/refetch lifecycle.

This optimization applies only to the expensive `headInfo`/workspace cache tag. Other watcher-driven queries keep their existing invalidation behavior.

## Placement

Production code exposes one shared Rust input snapshot in `but-graph` to graph construction, the Lite `head_info_snapshot` path, mutation response construction, and watcher event construction. None of those consumers independently reimplements the graph input set or encoding. `but-api` extends that snapshot with the reduced forge PR map when producing the revision. JSON/schema generation carries the contract through N-API into the SDK, and the Electron bridge remains a transparent transport.

## Performance gate

Benchmark warm executions of:

1. current `head_info`, including graph traversal, projection, expensive commit information, and forge association; and
2. `WorkspaceRevision` over the same repository state.

Report absolute time and the ratio across repeated samples in a release build. The prototype is successful only if revision computation is clearly cheaper on a representative non-trivial repository. The end-to-end success metric after wiring is fewer backend `head_info` calls following Lite mutations, with no stale workspace incidents.

If the checksum costs roughly the same as `head_info`, stop: the proposed optimization merely moves the work and should not be shipped.

### Prototype result

The prototype passed the initial performance gate on 2026-08-25. It was run in a release build against an isolated shared clone of the GitButler repository containing 2,396 refs, with three warm-up executions followed by 100 samples per operation:

| Operation                     |   Median |
| ----------------------------- | -------: |
| `WorkspaceRevision`           | 0.789 ms |
| current `head_info` lifecycle | 3.121 ms |

`head_info` was **3.96× slower**, so the checksum was about 75% cheaper in this run. The benchmark measures the existing projection lifecycle without the two coherence checks now wrapped around a direct `head_info` response. The repository had no representative GitButler project/forge metadata, so re-run on repositories with large active workspaces and populated metadata before treating the ratio as a production guarantee.

### Full endpoint result

The follow-up benchmark kept the original measurements, added the complete `head_info_snapshot` endpoint, and then measured a repository reload followed by revision computation. It ran in release mode in this active Conductor workspace, with 21 local/remote refs, three warm-ups, and 100 samples per operation:

| Operation                 |       p50 |       p95 |       p99 |
| ------------------------- | --------: | --------: | --------: |
| `WorkspaceRevision`       |  2.044 ms |  2.179 ms |  2.213 ms |
| reload + revision         |  2.484 ms |  2.643 ms |  2.747 ms |
| shared `head_info`        | 103.621 ms | 109.701 ms | 129.091 ms |
| full `head_info_snapshot` | 109.855 ms | 123.930 ms | 147.247 ms |

At the median, reload added 0.440 ms, making reload plus revision **1.22×** the revision alone. The revision remained **50.68× cheaper** than `head_info`, and the complete snapshot endpoint was **1.06×** the cost of `head_info`. This active-workspace result is substantially more favorable than estimating endpoint overhead from the isolated-clone measurements, but both datasets remain visible because workspace topology dominates `head_info` cost. It does not establish reload cost on repositories with large configurations, alternates, or expensive object-database setup.

After centralizing the semantic input snapshot, a 20-sample release-mode sanity run in the same workspace measured revision p50 at 2.169 ms, `head_info` at 109.129 ms, and the full snapshot endpoint at 113.356 ms. The revision remained **50.32× cheaper** than `head_info`; retain the 100-sample results above as the more representative dataset.

## Verification

The focused Lite unit test exercises the decision table: matching revisions preserve `headInfo`, while different or missing revisions retain the old invalidation behavior. It also verifies that matching revisions do not suppress other event-driven query invalidations, and covers both watcher-before-mutation and mutation-before-watcher ordering.

The E2E test uses the existing remote-branch fixture and real Electron/N-API bridge:

1. Wait for the initial workspace to render and record the number of `headInfo` IPC calls.
2. Create a new workspace branch through the UI. The mutation response updates the query cache.
3. Wait for the resulting watcher batch and assert that the new branch renders.
4. Assert that the `headInfo` call count did not increase.

The call counter is an Electron-main log emitted only when the existing E2E environment variable is present. It observes the real IPC endpoint without adding a test API or changing runtime behavior in normal builds. Revision mismatch and null fallbacks remain cheaper and more deterministic to cover in the unit test.

## Deliberate first-version limits

- No causal mutation ID.
- No global database generation counter or SQLite `data_version`; unrelated rows would create false misses.
- No watcher-wide suppression window.
- No ordering semantics or comparison other than exact string equality.
- No Gerrit input.
- No attempt to optimize every watcher tag in the first change.
- The snapshot consumes semantic workspace and branch metadata through `RefMetadata`; it does not depend on the transitional `virtual_branches.toml` storage format.
- Symbolic refs under `refs/heads/*` or `refs/remotes/*` are hashed by target name, not peeled object ID. Their usual targets stay inside those hashed namespaces, but a symbolic branch targeting a tag or custom namespace can change its resolved commit without changing the revision. Supporting arbitrary symbolic branch targets is deliberately out of scope; add this limitation to the checksum implementation's doc comment.
- Lite's `head_info_snapshot` computes the revision before and after traversal and returns `null` if the two reads disagree.

### Mutation-response coherence

The graph input inventory and canonical encoding now live together in `but-graph`. Workspace construction captures the inputs before and after traversal and only carries the snapshot through materialization when both reads match. Mutation response construction performs one final comparison against live inputs and returns `workspaceRevision: null` if the source snapshot is missing, changed, or cannot be read. The exact forge PR association map applied during projection is then added to the versioned hash.

Callers that modify branch order, workspace metadata, project metadata, or target refs after materialization perform one final shared workspace refresh after those writes. Other non-preview mutation paths that supply an already-built workspace are rebuilt through the same shared helper before projection. This prevents pairing projection A with revision B while preserving the existing safe fallback when coherence cannot be established.

### Known generic-operation deferral gap

Lite defers a workspace watcher comparison while a matching workspace mutation is pending, so the mutation response can update the cached projection and revision first. The generic `useExecuteOperation` path is not currently recognized because it declares neither `meta.updatesWorkspace` nor a project ID in its mutation variables. A watcher event can therefore invalidate and refetch `headInfo` immediately before the operation response writes the same workspace into the cache.

Fix this by marking generic operations as workspace-updating and carrying their project ID in mutation metadata, then let the pending-mutation check use that metadata when the variables do not contain a project ID. Add a watcher-before-response test through the generic operation path; matching revisions must avoid `headInfo` invalidation.

### Watcher performance and repository reload investigation

The watcher captures a long-lived thread-safe repository context. Revision computation rereads refs, project metadata, and app settings, but configured remote names and branch tracking mappings come from the repository's configuration snapshot taken when that context was opened. A focused repository-backed probe confirmed that both `remote_names()` and `branch_remote_tracking_ref_name()` remain stale after editing repository-local config and observe the new values only after `Repository::reload()`.

The event side is also incomplete. The file monitor watches the Git directory but deliberately classifies `.git/config` as uninteresting, and the watcher handler has no config path case. A config-only remote or tracking change therefore emits no workspace event. Reloading on existing Git/workspace events would correct a later event's checksum, but would not make the config change itself visible. Any eventual fix must first route relevant config changes to one workspace refresh, then use a fresh/reloaded repository for that event. Keep reload out of the shared revision computation.

The first reload benchmark is recorded above. Its 0.440 ms median overhead is small in this workspace, but validate against repositories with large or layered configuration before choosing reload over a narrower fresh-config read. No production reload is warranted from one repository sample.

The same investigation should measure revision p50/p95/p99, failures that fall back to `null`, adjacent watcher events that repeat the computation, matching revisions that suppress `headInfo`, and mismatches that trigger it. The broad checksum intentionally includes some refs and metadata outside the rendered projection; narrow or coalesce it only if end-to-end measurements show that watcher latency or false mismatches materially reduce the optimization's value.

## Follow-ups

1. Extend the focused repository-backed checksum coverage whenever graph inputs change.
2. Cover generic workspace operations in Lite's watcher-event deferral.
3. Validate reload cost on repositories with large/layered configuration, decide how relevant `.git/config` changes produce one fresh workspace event, and add temporary runtime counters before deciding whether duplicate calculations, fallbacks, or mismatches need optimization.
4. Re-run the benchmark against additional large active workspaces with populated forge associations.
