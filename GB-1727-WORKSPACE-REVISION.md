# GB-1727: Avoiding duplicate Lite workspace refreshes

## Outcome and implementation

Do not try to correlate a filesystem watcher event with the mutation that caused it. The watcher batches paths after a debounce window, external Git activity can overlap a mutation, and causal identity is lost before the event reaches Lite.

Instead, compare the state that determines the workspace projection. Rust now computes an opaque, versioned checksum called `WorkspaceRevision` and attaches it to direct `head_info` responses, materialized workspace mutation responses, and settled Git/workspace watcher events. Lite skips `head_info` invalidation only when the event revision exactly matches the revision already stored with its cached mutation/query result. A missing, failed, or different revision always refreshes.

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

Gerrit metadata is intentionally excluded.

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

Production code exposes one shared Rust computation to the direct `head_info` path, mutation response construction, and watcher event construction. None of those consumers independently reimplements the input set or encoding.

The implementation lives in `but-api`, the narrowest shared layer with access to project metadata, branch order, worktree state, and the reduced forge PR map. JSON/schema generation carries the contract through N-API into the SDK, and the Electron bridge remains a transparent transport.

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

## Verification

The focused Lite unit test exercises the decision table: matching revisions preserve `headInfo`, while different or missing revisions retain the old invalidation behavior. It also verifies that matching revisions do not suppress other event-driven query invalidations.

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
- Virtual-branch metadata is hashed as raw TOML. Formatting-only changes can cause a harmless extra refresh.
- Direct `head_info` computes the revision before and after traversal and returns `null` if the two reads disagree. Mutation responses derive the revision from the materialized graph plus current persistent inputs; an external Git writer racing that response assembly is not fully excluded. Capturing the canonical input snapshot in `but-graph` would close that narrow race, but is deliberately deferred rather than introducing a second graph-input model in this first version.

## Follow-ups

1. Add repository-backed checksum tests showing each included semantic input changes the revision and irrelevant Git storage rewrites do not.
2. Capture the canonical input snapshot during graph construction if the external-writer race is observed or this moves beyond an optimization hint.
3. Re-run the benchmark against large active workspaces with populated metadata and forge associations.
