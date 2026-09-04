# Vanilla git and the GitButler extension — a guided reading of the editor

A tour of `but-rebase/src/graph_rebase/` for one question: **which parts are a plain
git rebase engine, and which are GitButler's sauce?** Every claim below is a code
excerpt you can open. Read top to bottom; ~10 minutes.

The one-sentence version: the editor is a vanilla commit engine plus a store for
the one thing git cannot represent — **emptiness, and its order**. At every
altitude, something local tells you which world you're standing in.

The files, for orientation (all under `graph_rebase/`):

| File                          | World                                                                     |
| ----------------------------- | ------------------------------------------------------------------------- |
| `commits.rs`                  | vanilla — the commit graph and its surgery                                |
| `positions.rs`                | sauce — reads and checks over the extension's facts                       |
| `ref_ops.rs`                  | sauce — writes to the extension's facts                                   |
| `store.rs`                    | both — the composition; the seam lives here (§6)                          |
| `mutate/`                     | both — the verbs, each stating its consequences for both worlds           |
| `rebase.rs`, `materialize.rs` | the write boundary — git's ref edits derived from positions, then applied |

## 1 · The map (where you'd land first)

`graph_rebase/mod.rs`, module doc:

```rust
//! - `commits` — the vanilla half: the commit graph mounted for editing. It imports
//!   nothing from the ref side, so commit surgery cannot touch references.
//! - `positions` (reads and checks) and `ref_ops` (writes) — the extension: group order,
//!   carries, empty-lane slots; the facts git itself cannot represent. The split is the
//!   signature: every `positions` function takes `&EditorStore`, every `ref_ops`
//!   function takes `&mut`.
//! - the verbs (`mutate`) — deliberately both worlds: a mutation states its commit-side
//!   and ref-side consequences in one place, because the right ref-side consequence
//!   depends on what the mutation means. There is no fixup pass anywhere.
```

And the degenerate-case story that makes the two worlds one model:

```rust
//! Vanilla behaviors are the extension's degenerate cases: a plain ref is a singleton
//! group with carry `All` and nothing stacked above it, and a ref following a rebase is
//! implemented by doing nothing — positions stand still while ids rewrite underneath.
```

## 2 · A mutation, line by line (the reading rule)

The `insert_commit(…, Above)` arm in `mutate/insert.rs`. Every line classifies
itself by spelling:

```rust
let new_idx = self.store.commits.add_commit(spec);              // vanilla surgery
let target_commit = commit_entry(target)?;
self.store.commits.redirect_children(target_commit, new_idx);   // vanilla surgery
self.store.commits.push_parent(new_idx, target_commit);         // vanilla surgery
ref_ops::reposition_refs(                                       // extension write —
    &mut self.store,                                            //   the vanilla fact
    target_commit,                                              //   rides inside (§6)
    new_idx,
    ref_ops::Carry::Preserve,
);
```

The rule: **`store.commits.…` is vanilla; `ref_ops::` / `positions::` is the
extension; a bare `store.…` method does not self-classify — its own doc says what
it serves or spans** (see §6). And a question the exhibit should raise: the last
line also moves branches onto the new commit — a git-visible change — so why is
there no vanilla line for it? Because the extension's fact contains the vanilla
one: which commit a name points at falls out of where the ref stands (§6). One
write covers both, so there is no second line to show.

## 3 · How the worlds compose — four shapes, all of them

**a. The textbook sequence** — §2 above: vanilla surgery states its facts, then the
extension is brought along _in the same breath_.

**b. The free case** — `insert_commit(…, Below)`: vanilla calls only, and the
extension line you'd expect _does not exist_:

```rust
let target_commit = commit_entry(target)?;
let new_idx = self.store.commits.add_commit(spec);              // vanilla
self.store.commits.transplant_parents(target_commit, new_idx);  // vanilla
self.store.commits.push_parent(target_commit, new_idx);         // vanilla
```

Carry statements name parent entries by _stable id_, so when the whole parent array
transplants, the extension's facts follow without a single maintenance line.

**c. The interleaved case** — `split_group_with_commit` (inserting a commit between
a commit and the branch pointing at it): extension → vanilla → extension, and the
order is load-bearing (the group must be captured before the fresh parent entry
exists):

```rust
let new_idx = self.store.commits.add_commit(new);            // vanilla
let (split, on_commit) =
    self.interpose_into_group(target, new_idx, boundary)?;   // extension: capture,
                                                             //   split, redirect
self.store.commits.push_parent(new_idx, on_commit);          // vanilla — after the
                                                             //   capture, not before
ref_ops::settle_group_lower(                                 // extension: the split-
    &mut self.store,                                         //   off slice re-anchors
    &split.lower,
    ParentEntry { child: new_idx, number: 0 },
);
```

This is why there is no "vanilla editor plus a fixup step at the end": the two
streams constrain each other's _ordering_ inside a single verb.

**d. The named handoff** — `remove_parent` in `store.rs`, one line per world with
the seam in the doc:

```rust
/// Every other parent mutation belongs to the commit graph alone; this one also
/// drops ref-side statements, which is why it lives on the store.
pub(crate) fn remove_parent(&mut self, child: CommitIndex, parent_number: usize)
    -> Option<CommitIndex>
{
    let (target, removed) = self.commits.remove_parent(child, parent_number)?; // vanilla
    self.retain_edges(|&id| id != removed);                                    // extension
    Some(target)
}
```

## 4 · The ref lifecycle (where the vanilla mutations live)

Commit surgery is §2–3; branches themselves are made and changed like this:

| Mutation | Verb                                                         | Path                                        | World                                                              |
| -------- | ------------------------------------------------------------ | ------------------------------------------- | ------------------------------------------------------------------ |
| rename   | `Editor::rename_reference`                                   | `store.set_reference` (record op)           | vanilla                                                            |
| delete   | `Editor::remove_reference`                                   | tombstone (record) + `store.splice` (heal)  | vanilla + one extension line                                       |
| create   | `Editor::add_reference` (unpositioned) or `insert_reference` | record birth, then placement                | **split**: the name is born vanilla, the _place_ is born extension |
| repoint  | **no verb exists**                                           | `ref_ops::repoint_ref`, one internal caller | see below                                                          |

The repoint row: `git branch -f`, the most vanilla ref mutation of all, has no
verb — and none is needed. Refs follow rewrites by standing still while ids
rewrite underneath, so the product never needs to say "point X at Y"; deliberate
movement is `move_reference`, in group vocabulary. The one surviving `repoint_ref`
call is internal parent-surgery wiring.

And the word the insert verbs dispatch on: **`InsertSide` is vanilla only when both
operands are commits.** Above/below a commit is an ancestry choice git can express;
with a reference as an operand, "above" and "below" speak the extension's language
— git has no up or down among refs on one commit.

## 5 · The theorem you can check yourself

The _entire_ external surface of `commits.rs` — the vanilla half:

```rust
use but_core::commit::SignCommit;

use crate::graph_rebase::{
    CommitSpec,
    cherry_pick::{PickMode, TreeMergeMode},
};
```

No ref table, no positions, no ref_ops. "Commit surgery cannot touch references" is
not a comment — it is an absence the compiler maintains. The extension's mirror is
§1's signature rule: reads take `&EditorStore`, writes take `&mut`.

## 6 · The seam: one fact crosses, and it is checked

`RefState` in `store.rs` — the one thing both worlds touch, with the doc naming the
whole arrangement:

```rust
/// The commit this reference stands on — the vanilla fact (name → commit, what git
/// itself can say), written by the one primitive `set_on`, which only the
/// placement functions call. The extension's layout table annotates this fact (order among co-located
/// refs, carries); `assert_positions_total` checks the two agree at every mutation
/// exit, and `locate` reads the fact instead of scanning the table.
pub on: Option<CommitIndex>,
```

And the check, running at every mutation exit in debug builds (`positions.rs`):

```rust
fn assert_table_annotates_on(store: &EditorStore) -> anyhow::Result<()> {
    for (entry, name, key) in store.ref_positions_for_check() {
        let scanned = store.locate_by_scan_for_check(name.as_ref()).map(|(k, ..)| k);
        if key != scanned {
            anyhow::bail!(
                "table annotation contradicts the vanilla fact for {name} ({entry}): \
                 record says {key:?}, table says {scanned:?}"
            );
        }
    }
    Ok(())
}
```

At the write boundary: git receives only what git can hold. The ref transaction is
_derived_ from positions after ids rewrote in place — "rewrote the graph but forgot
to move the branch" is not a bug this engine can have; there is no such step to
forget. The same derivation retires metadata: a name the transaction deletes drops
its per-branch metadata at materialize, so "deleted the ref but kept its dead PR
association" cannot happen either.

## 7 · The subtraction test (how far is this from a vanilla editor?)

Measured 2026-08-05, on the question: how many lines change to make this a pure
vanilla editor — plain name→commit refs, no emptiness, no order?

| What                                                                        | Lines       |
| --------------------------------------------------------------------------- | ----------- |
| delete two files by name (`ref_ops.rs`, `positions.rs`)                     | 1,090       |
| store.rs sheds the placement machinery, layout table, and minted ws-parents | ~400 of 827 |
| creation.rs sheds layout ingest and real/minted parent classification       | ~275 of 493 |
| insert.rs sheds the group choreography; `insert_reference` collapses        | ~180 of 583 |
| grep-deletable `ref_ops::` / `positions::` call lines elsewhere             | ~40         |

About 2,000 of the module's 10,800 lines — 18% — and all of it subtraction. The engine —
`commits.rs`, `cherry_pick.rs`, `rebase.rs`, `materialize.rs` — changes by zero
lines: `derive_ref_edits` already reads only the vanilla fact. (File deletions are
exact; mixed-file counts are function-span measurements, marked ~.)
