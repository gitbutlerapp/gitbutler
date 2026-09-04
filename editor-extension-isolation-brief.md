# Isolating the GitButler extension in the editor

**Acceptance criterion (the deletion test):** delete the GitButler-specific lines and a
working plain-git rebase editor remains — commits, refs that stand on them and follow
rewrites, materialization. Today that test fails at exactly one structural point.

## Where the line stands after the store campaign

| Layer                                     | Status                                                               |
| ----------------------------------------- | -------------------------------------------------------------------- |
| commit half (`Commits`, commits.rs)       | isolated, import-pure — commit surgery cannot touch refs             |
| reads vs writes (`positions` / `ref_ops`) | signature-checkable law: `&` vs `&mut EditorStore`                   |
| call sites                                | every beyond-vanilla line wears a `positions::` / `ref_ops::` prefix |
| **a ref's `on`**                          | **inside the GB layout table — the failure point**                   |

A position is _membership in a group at a key_: `positioned_on` = `layout.locate(name)`,
a linear scan of every site, group, and member. Delete the layout and you don't just lose
order and carries — you lose "where does this branch point," which is git's own fact.
The vanilla core is implemented _through_ the extension.

Census: **79 vanilla-class read sites** (`positioned_on`, `resolve_to_commit`) route
through that scan; 107 extension-class reads (`entering`, `ref_depth`, `below_of`,
`carry_of`, `group_members`) legitimately need the table. And `locate` is the code's own
flagged debt: "THE HOT PRIMITIVE … if a workspace ever grows into the hundreds of
branches, this is the primitive to index." The isolation move and the de-linearization
are the same refactor.

## The design

**Vanilla truth moves onto the record.** `RefState` gains `on: Option<CommitIndex>` —
name → commit, the fact git can say. Tombstone descent (`resolve_to_commit`) is
unchanged and already vanilla (dropped commits are vanilla rebase behavior).

**The layout becomes an annotation.** The table keeps order-among-co-located-refs,
carries, and below-chains — the facts git cannot say — but stops being the home of `on`.
`locate` becomes: read `on` (O(1)), then scan only that commit's groups.

**One placement door maintains both.** `place`/`extract` already gate every position
write; they set `on` and the table together. The law gains a clause:
`assert_positions_total` checks `on == locate(name).key` for every positioned ref —
the sync invariant is continuously verified, not trusted.

**Rider policy gets named as policy.** Which refs follow which surgery (ride up on
interpose, stay with the lineage on move, checkout follows its commit) is product
policy, not mechanics. The coherent vanilla default is _refs stay put while ids rewrite
underneath_ — which costs zero code. The GB rider rules layer on top, where
`ref_ops::reposition_refs` already names them.

**The vanilla subset then is closed:** `Commits` + the ref record (`RefState` + `on` +
`by_name`) + `resolve_to_commit` + repoint/rename/delete + the rebase loop +
`derive_ref_edits` + materialize. Deleting {layout table, group surgery, carries,
ws slots, mint filter, ambiguity} leaves it compiling and correct.

## What deliberately does NOT isolate

- **The verbs** (mutate/\*): plans legitimately compose both halves — the Carry lesson
  (maintenance policy is a function of intent) is architecture, not debt.
- **Anchors and the well-formedness checks**: their job is the join.

## RULING (reader-first revision)

The goal is READER comprehension, not a shippable vanilla editor — so the deletion test
is the wrong acceptance criterion for the remaining work. Rungs 2–3 below are PARKED:
they are proof machinery, and their real trigger is the deletion test becoming a product
goal (e.g. extracting a reusable rebase crate). What replaced them:

- **R2′ — the two-worlds doc pass**: the vanilla/extension map, the degenerate-case
  story, and the mirror-as-seam stated in `graph_rebase`'s module docs where readers
  land, with one-line world-banners on `positions` and `ref_ops`.
- **R3′ — the rider rules stated**: which refs follow which surgery, as prose beside the
  writes that implement it, with the vanilla default (stay put, zero code) explicit.

The trap this avoids: mistaking proof machinery for pedagogy — code that _proves_
something a reader never asks, instead of _saying_ what they need.

## THE SPARSE-OVERLAY DESIGN (supersedes the vanilla-map sketch below)

Ratified 2026-08-04: the purest extension shape is not "vanilla store + extension
store" but "vanilla store + a SPARSE OVERLAY recording only deviations from vanilla" —
absence means vanilla. Goal is structural purity, not configurability: no traits, no
feature flags.

- **`on` is the primitive write**; the layout annotates it. (The mirror from rung 1
  becomes the truth it was always shaped like.)
- **The table stores deviations only**: multi-member groups, `Entries` carries, attach
  chains, and `None`-carry root groups (dead-retained refs, empty tops — already
  fold-exempt today). CANONICAL DEFAULT for an absent ref: singleton group, carry
  `All` — the ordinary branch-label shape. A plain ref alone on its commit has NO table entry — it is a derived
  singleton with carry `All`. The degenerate defaults already exist in the code as
  fallbacks (`unwrap_or(GroupCarry::All)`) and normalization targets (the aliasing
  rule); sparsity makes them the storage rule. Refs promote into the table on first
  stacking and degenerate back out.
- **The totality law inverts into a sparsity law**: every table entry must agree with
  `on` and be non-degenerate; absence is vanilla by definition. Deletion test passes by
  construction — an empty overlay is a valid overlay meaning "plain git".
- The extension's stored state becomes exactly GitButler's diff against vanilla,
  materialized as data.

Rungs: (1) door inversion, table still total — `set_on` the named primitive, law
direction flips [DONE on branch]; (2) vanilla verb paths — doors write the fact first,
annotation second; (3) sparsify — derived defaults, law inversion, promote/degenerate
lifecycle, fuzzer-gated; (4) ingest split — git refs feed the record, RefLayout feeds
the overlay.

**RUNG 3 HALTED (2026-08-04, simplification audit).** Implementation was started and
reverted after slices 1–2: it grew a promote/demote lifecycle, an attach-target guard,
fold-twin materialization, and a liveness-split default — a second representation of
one state plus permanent machinery to keep the two honest, with nothing deleted in
exchange. By the campaign's own method (refactors must reveal themselves; this one was
accumulating special cases) rung 3 is purity machinery, not simplification. The reader
goals were already met by the naming/docs/mirror work. Re-execute only on a product
trigger (reusable vanilla engine); the design above plus the canonical-default ruling
is sufficient to restart in an afternoon. Refinement discovered during implementation:
absence defaults by LIVENESS — live → singleton `All`, dead → retained `None` — which
lets dead retained positions sparsify too.

## Rungs (original ladder; superseded by the sparse-overlay design above)

1. **`on` + dual-write door + law clause + `locate` fast path.** Behavior-neutral,
   kills the O(R²) tripwire. Small: `RefState`, `place`/`extract`, `locate`,
   `assert_positions_total`.
2. **Split the ledger's fields**: `RefLedger` → vanilla ref record vs layout annotation,
   mirroring the store split; vanilla ref ops (`repoint_ref`, rename, delete resolution)
   read only the record.
3. **Provable purity**: the vanilla ref module imports nothing from the layout —
   the record-side sibling of the commit half's import theorem. `derive_ref_edits`
   compiles against the vanilla subset alone.
4. _(Optional)_ Name the rider policy: a small enum at the reposition seam, with the
   vanilla default explicit.

## Costs, honestly

- **Dual-write risk**: `on` and the table can disagree. Mitigated by the single door +
  the law clause; the fuzzer's position laws exercise it continuously.
- **Partial reversal of the PositionTable unification**: what that unification bought —
  one placement algorithm (`place_in_groups`) shared with the builder, one stored shape —
  stays. Only the `on` coordinate gains a direct home; the editor side stops deriving a
  vanilla fact from an extension structure.

## Recommendation

Rung 1 is small, behavior-neutral, and pays for itself in performance — it could land on
this branch as another revertible commit. Rungs 2–4 are the post-merge campaign, with the
deletion test as the acceptance criterion.
