# Verification side effects: prevention and cleanup

The selective-copy scenario reveals a consistent provider difference that is obscured by counting only Git and But commands. Across the **29 completed canonical repetitions per provider**, Astra's submitted tests/verification always use Python's `-B` no-bytecode mode. Fable usually runs tests normally and then removes the generated cache. All eight completed-study residue failures occur in the Fable group with neither an explicit no-bytecode test command nor workspace cache cleanup. [1]

| Observed submitted strategy | Astra trials | Fable trials | Final state checks |
|---|---:|---:|---|
| No-bytecode mode in test/verification command | 29 | 1 | All pass |
| Explicit workspace `__pycache__` cleanup, no no-bytecode mode | 0 | 18 | All pass |
| Tests only in extracted temporary tree, then remove that tree | 0 | 2 | Both pass |
| Neither suppression nor workspace cache cleanup | 0 | 8 | All fail `unfinished_only` |

This is a description of one repeated scenario and its unchanged Python fixture. It does not justify putting a Python flag or cache-directory exception into the GitButler skill. It does show that “verify, then clean up” is not the only successful implementation of the real requirement: supplementary verification must preserve the requested final workspace state.

## Evidence detail

Twenty-three Astra trials submit the literal `python3 -B test_commands.py`. The other six were manually inspected: four execute the branch's test code in memory under `python3 -B`, and two extract the relevant files into a `TemporaryDirectory` and run a child `python3 -B` there. In two of the six trials the first interpreter spelling is unavailable `python -B`; the following attempt uses `python3 -B`. The no-bytecode strategy is therefore not a hidden environment assumption inferred solely from a clean final state. It is visible in the submitted commands. These six nonliteral cases are:

- September 8 baseline, selective-copy r2, `item_14`.
- September 8 baseline, selective-copy r5, `item_15`.
- September 10 unmodified stub, selective-copy r2, `item_8` then `item_9`.
- Minimal-guidance i01, selective-copy r1, `item_11`.
- Minimal-guidance i04, selective-copy r1, `item_10`.
- Minimal-guidance i06, selective-copy r2, `item_9` then `item_10`.

The two Fable cases that succeed without a visible cache-removal command are September 10 unmodified-stub r2 and r3. They run tests inside a `git archive` extraction and remove the exact temporary directory afterward. They do not run those tests in the main workspace. They are an important counterexample to equating absence of `rm __pycache__` with failure.

The stopped i07 run is excluded from the table. Its Fable selective-copy r2 supplies a useful failure sequence: inspect workspace status; run workspace tests; run tests in an extracted temporary tree; remove temporary extraction; finish. The initial status precedes the cache-producing workspace test. The extraction is cleaned, but the workspace cache remains. This reproduces the earlier order problem despite the candidate still containing cleanup guidance. The command uses a broad `/tmp/rwng.*` cleanup glob; the trace does not establish damage to any other file, but it also does not demonstrate exact ownership of every matched path. [2]

## Scope and limits

The regex scan identifies candidates in actual submitted tool calls, not new native instrumentation. The six nonliteral Astra cases and the two temporary-only Fable cases were reviewed to interpret their execution scope. Canonical final-state checks are retained from the original trials; no outcomes are rescored. The linked corpus also contains the raw native/tool output evidence for deeper verification.

Model, instruction, product and sequential-study differences prevent causal attribution of the strategy difference. The evidence does not prove that a cleanup sentence caused the successful sequences, that omission alone caused every residue failure, or that Astra universally handles side effects better. It identifies a concrete completion property and multiple successful ways of satisfying it.

## Sources

1. [Candidate extracts](/Users/kiril/tmp/but-skill-evidence-2026-09-10/data/copy-side-effect-candidates.json): per-trial source path, event ID, command input, study stratum and original state outcome. [Extraction script](/Users/kiril/tmp/but-skill-evidence-2026-09-10/data/mine-copy-side-effects.py), drawing from [benchmark trial rows](/Users/kiril/tmp/but-skill-evidence-2026-09-10/benchmark/trials.json) and [normalized submitted tool calls](/Users/kiril/tmp/but-skill-evidence-2026-09-10/benchmark/tool-events.jsonl).
2. [Stopped i07 selective-copy r2 capsule](/Users/kiril/tmp/but-skill-evidence-2026-09-10/benchmark/selected-evidence/stub-guidance-20260910-i07--stub-claude-warm-024-pilot-13-selective-copy-claude-butplusskill-r2.json), final call `toolu_018dKrJBzNNKumW3HfJuiKt2`. The final response claims tests and preservation passed; the actual failed check is `unfinished_only`.
