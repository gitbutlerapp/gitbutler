# Intentional graph-behavior changes (CommitGraph flip)

As part of replacing the SegmentGraph walk with the CommitGraph-derived build, we
are **deliberately simplifying** several graph-shaping behaviors. These are not
bugs in the flip — they are decisions to drop walk behaviors that add complexity
without clear value. Recorded here for the eventual commit message / PR body.

## 1. No duplicate parents in workspace merge commits
The GitButler workspace merge previously encoded empty lanes as **repeated
parents** (e.g. `[base, base]`). We abandon that as a *design* idea: lanes are
derived from workspace metadata, not the parent array. Both the walk and the
flip collapse exact duplicate parents at read time
(`CommitGraph::from_repository_seeded`: `[X,X]→[X]`, keeping `[X,Y]` and
`[X,Y,X]→[X,Y]`), so a merge with repeated parents renders as if it had the
distinct set.
- **Effect:** no visible change to the rendered graph.
- **Tests:** `duplicate_parent_connection_from_ws_commit_to_ambiguous_branch`
  and its `_no_advanced_target` sibling KEEP their fixtures with duplicated
  parents and now assert the *deduped* outcome (the branch is visible once).
  They stay as defensive coverage that dup-parent commits in the wild are
  handled — they are not the abandoned "two lanes from dup parents" behavior.

## 2. The entrypoint segment is named, not forced anonymous
When checked out *into* a stack (e.g. on branch `lane`), the walk forced the
entrypoint segment **anonymous** and floated the branch as a separate empty
segment above it. We drop that: the checked-out branch simply names the segment
it sits on.
- **Before:** `anon:` holding the commit + a floating empty `👉lane` above it.
- **After:** `👉lane` names the commit's segment directly.
- Note: a *truly detached* HEAD (no ref) stays anonymous — there is no branch to
  name it. Only the "checked-out branch" case changes.

## 3. No separate empty segment for the target's local branch
The walk emitted a floating empty segment for the target's local branch (`main`)
purely to mark where the target rests (`ensure_local_tracking_segment_for_remote`).
We drop it: `main` is a plain ref on the commit, and the remote attaches as a
normal sibling of the real segment.
- **Before:** a standalone `main <> origin/main` segment with no commits.
- **After:** `main` is a ref on the commit; `origin/main` is the sibling of the
  segment that owns the commit.

## 4. The target does not get naming priority; it is treated as a remote/sibling
At a commit shared by the target's local branch and a workspace stack branch,
the walk named the segment after the **target** (`main`) and floated the stack
branch empty. We drop the special case: naming is uniform — a workspace stack
branch (metadata) names the segment; the target is just a ref + remote sibling.
- Disambiguation of *which local branch names a shared commit's segment* is
  **remote-tracked branch → the only branch → anonymous** (the flip's
  `disambiguated_ref`). This is NOT a simplification — it reproduces the walk's
  remote-local-tracking naming (the walk names a segment after the local branch
  whose remote points at that commit, e.g. `main` for `origin/main`). An earlier
  note here claimed a "metadata → remote → single" rule; that was wrong — there
  is no metadata tier, and switching to one regresses parity.

## 8. `InWorkspace` is message-driven, not metadata-driven (KEEP FLIP)
A `gitbutler/workspace` checkout whose merge commit has the managed-workspace
message marks its reachable commits `InWorkspace` (`🏘`) even when there is **no
workspace metadata**. The walk instead treats a no-metadata workspace checkout as
a normal branch (no `🏘`) — see `minimal_merge`'s "the branch is normal!" comment.
Decision (Mattias): **keep the flip** — the message is the more robust signal in
production where metadata can lag. The walk is NOT retrofitted (reclassifying a
no-metadata checkout as a `TipRole::Workspace` tip would reshape target
resolution + stack materialization in a soon-to-be-deleted codebase); the
affected walk snapshots (`minimal_merge*`, `without_target_ref_*managed_commit*`,
…) are re-accepted to the flip's output when the flip becomes the default.

## 6. Worktrees are KEPT (CLI supports them)
Not a simplification — the flip must reproduce the walk's worktree annotations:
the main worktree `[🌳]`, linked worktrees `[📁]`, and the `@repo` ownership
marker (which worktree the building repo owns). This is a feature the flip needs
to add, not remove.

## Deferred (no decision yet)
- **#5 inactive/outside stacks as empty segments** — kept as-is for now (the
  metadata is retained for PR/branch-info survival; the projection filters them).
- **#7 shared remote-ahead ownership** — when one commit carries two remote refs,
  how they stack. Deferred.
