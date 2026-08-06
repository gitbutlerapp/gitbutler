# Refs as positions: the mutation-complexity evidence

_The measured argument behind rule 5 of `WORKSPACE_MODEL.md`: modeling references as
**positions on commits** (the current editor) does not burden the mutation functions with
heavy complexity — despite the intuition that refs-as-**nodes** (the pre-rewrite
`graph_rebase` of 2026-08, "master" below) would make mutations simpler, since "a ref is
just another node and moving it is generic node surgery."_

_The comparison is like-for-like: the same crate, the same operations
(`disconnect`/`insert`/`replace`), two models. The baseline editor was petgraph nodes for
commits AND references; the current one is a commit arena plus a position table._

## The intuition, and where it breaks

Refs-as-nodes makes one function look simpler: master's `disconnect_segment_from`
(mutate.rs:374) is pure edge surgery and **never mentions references at all**. That silence
is the intuition's whole appeal — and its defect. A ref node sitting between two commits is
rewired by whatever the edge topology happens to do; **the decision about what a mutation
means for a ref is made nowhere**. The complexity does not disappear. It disperses into
three taxes intrinsic to the model — plus implementation costs our particular baseline
added, kept separate below so the argument stays honest:

### Tax 1 — the fundamental read becomes a graph search

_Scope note: this and the following taxes hold for the version of refs-as-nodes the
intuition actually defends — refs **interposed in the ancestry edge structure**, since that
interposition is precisely what lets generic edge surgery move a ref and what expresses
stacked-ref order topologically. Refs as out-of-line decorations avoid these costs, but
then edge surgery cannot move them and order needs separate machinery — a positions model
wearing node syntax._

On master, "what are this commit's parents?" requires `collect_ordered_parents`
(util.rs) — a **pruned depth-first search with a seen-set** — because real parents hide
behind chains of `Step::Reference` and `Step::None` nodes. The rebase replay calls it per
commit; anything that forgets to gets ref nodes as parents.

Ours: a commit's parent list **is** its parent list — a stored array in the arena
(`commit_graph.rs:662` is a `filter_map` over stored entries). References cannot appear in
it by construction; there is no flattening walk anywhere.

### Tax 2 — uniform edges force no decisions

Connectivity doesn't know what it connects: edge surgery around a ref node **compiles
without anyone deciding what it means for the ref** — which is how a correct-looking
`disconnect` can never say the word "reference". The model doesn't forbid ref-aware
mutations; it makes them optional, and optional decisions get skipped. Whoever needs refs
to be _somewhere specific_ afterwards must reconstruct: master's workspace.rs carries
**71** reference-handling mentions, and even _finding_ a step's refs is a per-query
traversal (`step_references`).

Ours inverts the affordance: refs have no edges, so commit surgery **cannot** touch them,
and the only way to affect one is a named ref operation (`place`, `splice`, the group
split). The decision isn't diligence; it's structurally unavoidable.

### Tax 3 — ancestry gains members git doesn't have

An interposed ref — and every **empty branch** — is an ancestry participant with no git
object behind it. Every walker must define what stepping through one means, and the
rebase/materialize boundary must erase them — resolve refs to their nearest real commit
("References should have at least one parent", rebase.rs:118) and re-emit them. The
failure mode of forgetting is silent: a ref surfacing as a real parent of a written commit
corrupts the repository. The empty branch is the model at its most fictional: a node with
no commit, holding a place in ancestry git has no object for.

Ours: git's object model is stored as git defines it — commits name parents, refs are a
name plus a place. Materialization writes positions; nothing fictional was stored, so
nothing needs erasing.

### What the baseline added on top (implementation, not model)

Two further costs are real in master but **not consequences of refs-as-nodes** — a nodes
editor over a tombstoning arena would avoid both. They are kept out of the model argument
deliberately:

- **Identity went stale.** petgraph indices die on node removal, so every operation opens
  with `history.normalize_selector` ceremony (18 calls in mutate.rs alone) over a
  `SelectorHistory` remapping layer. Our tombstone discipline — nothing is ever deleted —
  is what removes this, and it could be applied to either model.
- **Placeholder nodes.** `move_commits` plants `Step::None` stand-ins so refs anchored by
  adjacency survive a commit's departure — one implementation's fix for a genuine model
  friction (ref position expressed as adjacency means commit mutations disturb refs), but
  a choice, not an entailment.

## What refs-as-positions actually costs

The premium is real — and it is **concentrated, named, and verified** instead of dispersed
and implicit:

- `positions.rs` (~380 lines): the position store's derived reads (groups, depth, entering,
  `resolve_to_commit`) plus the well-formedness assertions every mutation exits through, and
  the glossary naming the concepts (carry, statement, rider).
- Inside the mutation module, position handling appears only at named seams: the lone-ref
  shortcut in `disconnect` (`unhook_ref`), `place_reference` and the group-split boundary in
  `insert`, `splice` on removal. The commit-side surgery — plans, severs, healing, parent
  renumbering — never mentions references outside those seams.
- The verbs' signatures are namespace-honest: `move_range` knows nothing about refs;
  `move_reference` is its typed single-reference form. A caller is never asked to think
  about ref plumbing to move commits.

Two empirical anchors:

1. **The week-long simplification campaign never found position machinery to delete.** What
   fell — four handle types, the `Step` union, `kind_of`, a dead ordering parameter,
   `disconnect_all`, an entire hallway module — was type ceremony and dispersal, not the
   position model. Three consecutive analysis passes converged to zero with
   place/splice/carry untouched: the position code is load-bearing, not fat.
2. **Capability per line runs the right direction.** Our mutation module is larger than
   master's (1,619 vs 902 lines), and it does strictly more: ordered ref groups with stacked
   empties, carries that survive rewrites, typed phase results, exit-verified
   well-formedness, and the `move_range`/`base_of` verbs that absorbed caller-side
   choreography out of but-workspace. Master's smaller file exports its missing decisions to
   every caller — the lines exist either way; the models only choose where.

## Measuring the exported complexity

"Exported complexity" is countable. Fix a product operation that exists against both
editors (but-workspace's consumer files match almost one-to-one across master and this
branch), then count four things on the caller's side:

| Metric                              | What it counts                                                   | Master (nodes)                                                                                                    | Ours (positions)                                                |
| ----------------------------------- | ---------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------- |
| **M2: representation leakage**      | caller-side matches on the editor's raw storage types            | **93** `Step::` matches + 3 petgraph/`NodeIndex` in but-workspace                                                 | **0** (`Step` doesn't exist; `EditorIndex` payloads are sealed) |
| **M3: caller-side surgery helpers** | helper LOC that exists only to prepare/repair editor calls       | `graph_manipulation.rs`, **433 lines**                                                                            | **deleted** (absorbed by `move_range`/`base_of`)                |
| **M4: primitives per operation**    | distinct editor calls to complete one product op (move a commit) | **5** (`order…`, `direct_parents`, `replace`, `insert`, `remove_edges`+`add_edge`)                                | **1** verb (`move_range`) after ordering                        |
| **M5: policy location**             | where the model's rules are implemented                          | in the caller — master's `move_commits` hand-splits first-parent from merge-side parents (`parents.split_off(1)`) | in the editor — `base_of`, one owner                            |

The `move_commits` exhibit deserves quoting in structure: master's caller reads raw parent
edges and implements the merge-parents-travel rule inline, replaces the subject with a
`Step::None` placeholder — justifying in its own doc comment why that is safe for refs —
then re-hangs merge parents edge by edge with `remove_edges`/`add_edge`.
Ours calls `move_range(Range::single(subject), …)`; the base rule, the ref behavior, and
the healing are the editor's, and the caller's file contains no statement about the model
at all.

Raw same-file LOC (M1) is deliberately **not** cited as evidence: our consumer files are
often larger because the branch versions handle more (empties reconciliation, workspace
dissolution, more shapes). Per-operation policy and representation counts are the honest
metrics; total line counts across feature-unequal branches are not.

## The same intent, told twice — a narrative exhibit

One visual intent: the workspace merge `W` has one parent, commit `C`; branch `A` points
at `C`. Insert a new empty commit `N` between `C` and the branch tip.

### On master (refs as nodes)

Stored: three nodes — `W` (`Pick`), `A` (`Step::Reference`), `C` (`Pick`) — and two
weighted edges, `W →₀ A →₀ C`. The load-bearing fact: **`W` reaches `C` through `A`** —
interposition is what makes the branch movable by edge surgery.

1. Write the object; wrap it as `Step::new_untracked_pick(new_id)`.
2. `editor.insert(relative_to, step, side)`, aiming `Above` `C`. Every selector first
   passes `history.normalize_selector` — indices go stale under mutation.
3. The `Above` arm removes **all** of `C`'s incoming edges (here `A →₀ C`) and re-adds
   each onto `N`, with the _chubbiest grand-child_ weight arithmetic (`max(order)+1`)
   dodging order collisions.
4. `add_edges_to_parents(N, [C], Prepend)` wires `N →₀ C`.
5. The branch moved — but **no step decided that**. `A` re-hung because it is a child of
   `C`, indistinguishable from any other child; a second stack parented on `C` would
   re-hang identically, wanted or not. That the ref follows must be _inferred_ from "all
   children re-hang."
6. `rebase()` replays: per commit, `collect_ordered_parents` (the pruned DFS) recovers
   real parents from behind ref/`Step::None` chains; ref nodes are erased from ancestry
   and re-emitted.

**Ledger:** interposition, edge weights + collision arithmetic, selector normalization,
the pick/step vocabulary, the all-children rule (ref's move implicit), reparenting order,
replay-time flattening/erasure — **seven concepts, three of them defenses of the
representation against itself.**

### After (refs as positions)

Stored: an arena with parent _arrays_ (`W.parents = [C]`) and a ref table where `A`
_stands on_ `C`; `W`'s parent entry carries the group `[A]`. Nothing reaches anything
_through_ `A`.

1. One call: `editor.insert_commit(anchor, CommitSpec::untracked(new_id), side)` —
   `Anchor` accepts the commit id, the ref name, or a held index.
2. The four-arm table `(side × is-the-target-a-reference)` decides, each arm a
   one-sentence rule written where it executes. Two honest spellings reach this intent:
   target `C`, `Above` — _"every ref sitting on it moves up"_ (children rewire with
   parent numbers preserved, `reposition_refs` lifts `A` onto `N`); or target `A`,
   `Below` — _"a commit below a reference splits the group at that reference"_.
3. Nothing at replay: the rebase reads parent arrays directly; refs are written from
   their positions; nothing fictional exists to erase.

**Ledger:** arena + parent arrays (order _is_ array position), refs-as-positions with
groups, `Anchor`, the spec, the four-arm rule table — **five concepts, every one a domain
decision, none a bookkeeping defense.** Indices never staling is not something to learn;
it is a concept that's absent.

### The asymmetry in one line

The nodes model **hides a decision** — the branch's movement is an unattributed side
effect of generic surgery. The positions model **demands one** — the caller must say
which side of the group they mean, and the four-arm table is that tax made explicit,
priced once, in one function.

## Verdict

Refs-as-nodes does not remove reference complexity — it removes the **language** for it.
The cost survives as implicit topology decisions, unstable identity, fictional ancestry,
boundary erasure, and per-query searches, spread across every reader and writer.
Refs-as-positions pays a bounded premium in two files with names on every concept, keeps
the fundamental reads O(list), stores exactly what git stores — and left the mutation verbs
simple enough that a week of aggressive simplification found everything _around_ them to
delete, and nothing _in_ them.
