# Deleting the walker's shadow segment model (Segs)

**Verified 2026-08-04** against walker.rs (1,231 lines): `Segs` (names / per-seg commit
lists / `of` map / split machinery, ~120 lines plus call sites) never escapes —
`WalkOutcome` carries none of it. But it has FOUR consumers, not the three the earlier
review recorded:

1. **Entrypoint naming** (traverse tail): `segs.of[ep] → names` — replaceable by the ep
   seed's own `ref_info`, EXCEPT the split-at-pos-0 edge (a fully split-off seed seg
   names the entrypoint `None` today). Arrival-order dependence lives here.
2. **`ep_first_flags`** (goal pacing fallback): first commit of the ep seed's segment —
   replaceable by per-seed first-collected tracking, same pos-0 edge.
3. **`attach_workspace_refs`**: per named seg, the first commit gains the RefInfo —
   same per-seed replacement, same edge.
4. **THE REAL CONSUMER — goal inheritance in `re_encounter`**: tips landing in LEAF
   SEGMENTS of a propagated visited cone (visited, no outgoing followed edges to other
   segs) inherit the arriving tip's goal. This is deliberately NARROWER than
   `queued_by ∈ visited` (that condition is the budget handoff, and its comment insists
   "never goals"). The goal protocols here are measurement-tuned (+22k regression
   history), so any commit-wise re-expression must prove equivalence.

## Campaign shape (two rungs, fresh block)

1. Extract consumers 1–3 onto per-seed tracking (small; battery-gated for the pos-0
   edges — snapshots pin entrypoint naming). Shrinks Segs to consumer 4.
2. Re-express the leaf-segment goal rule commit-wise, oracles: the full walk battery
   field-exact PLUS the gitlab-dump perf A/B (goals drive walk size — collect counts
   must match, not just outputs). Only then does the model delete.

**Do not attempt as a session-tail edit.** The rule of thumb from the rung-3 halt
applies: if rung 2 starts accumulating special cases instead of falling out, the
leaf-segment rule is telling you it IS the semantics, and the model stays with one
honest banner saying so.

## RUNG 2 OUTCOME (2026-08-05)

The goal-inheritance rule went commit-wise — "the queuing commit is in the propagated
cone AND is a frontier commit (followed parents not collected)" — via shadow
comparison: both rules computed side by side with a divergence assert, run against the
full battery (156), the op suites (507), and 3,000 fuzzed repositories. ZERO
divergences; flipped, and the leaf-segment machinery (landing, has_outgoing, the
BTreeSet scan) deleted. gitlab-dump stress repo: 0.64s warm vs the 0.74–0.78s recorded
baseline — parity-or-better, plausibly from losing the per-re-encounter scans.

The model survives with ONE consumer, stated in its banner: entrypoint ownership
(which name owns the entrypoint commit, evolving through splits and continuations —
the build cannot re-derive it). Full deletion is now precisely a PRODUCT decision:
change entrypoint-naming semantics ("naming leaves the walk") and Segs goes with it.
Engineering is done; the remainder is ratification.

## RUNG 3 OUTCOME — NAMING LEFT THE WALK (ratified + executed 2026-08-05)

Census: 73 walk-level ownership divergences; experimental flip showed exactly 5
product-visible diffs — four snapshots asserting a false `DETACHED` for a checked-out
workspace ref (unborn-workspace states), and one error message that can now name the
ref missing its base. Ratified: the entry is named by the requested ref, or (detached)
by the metadata ladder over refs at the entrypoint commit — both deterministic from
state. NOTE the ladder arm is load-bearing: the first implementation dropped it
("requested only") and `journey_anon_workspace` regressed to DETACHED — detached HEAD
parked on a branch tip is the ladder's case. The suite caught it; `entry_name_from_facts`
restores it.

Segs is DELETED — names, `of`, per-seg commit lists, `split_off_tail`, `sync_seeds`,
`segment_for_commit`, `seed_seg`, and the never-read `Instruction.new_segment` field
the compiler surfaced. The walker collects commits until the seeds converge and names
nothing. Oracles at deletion: but-graph 156, but-workspace 507 (4+1 ratified diffs),
but CLI 1,391, fuzzer 1,500 repos, gitlab-dump 0.65s warm (parity). Campaign closed.
