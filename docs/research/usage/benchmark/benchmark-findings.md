# Benchmark findings for a main-skill rewrite

The strongest evidence supports preserving the short task recipes while clarifying three boundaries: which output already answers the task, what Git’s index means in But mode, and responsibility for side effects of supplementary commands. More documentation or a blanket ban on verification is not justified. Successful two-command solutions coexist with deep audits; many audit checks correspond to explicit user invariants.

This is corpus mining, not another optimization iteration. i07 remains stopped. All conclusions describe observed tool interactions, outputs, timings and final checks; no hidden reasoning is inferred.

## Corpus and denominators

There are **1,254 distinct raw result records** on the VM. All have retained normalized events and native traces; no exact duplicate executions or raw count mismatches were found. The main But corpus is **754 complete canonical trials** (377/provider) plus **58 stopped-partial i07 results** (35 Fable,23 Astra). The all-arm canonical context is1,072 records: Sept8 all390, Sept9/Sept10 controls156, i01–i06 468, partial i07 58. Do not pool partial results with completed matrices.

Outside those denominators:21 canaries,140 prebaseline records (78 final,56 failed/retried attempts,6 background),12 model-validation trials and9 records from the abandoned early baseline. Two failed canaries are authentication/runtime failures with zero provider tokens, not task failures. They stay visible in the manifest and separate from canonical results.

The VM VCB checkout contains no historical raw result.json files. Its tmp trees are verifier fixtures, not model-run evidence. We retain27 report files (25 Markdown,CSV,JSON) as a report-only stratum. June/July reports cover changing five/six-scenario sets, older models and product versions; e.g July20 reports359/360 canonical passes with120/120 But, plus15 excluded infrastructure failures. The report’s missing raw transcripts prevent an independent behavioral audit. Their printed tables and provenance are retained without fabricating per-trial rows or double-counting report copies.

The independent frozen-predicate correction removes **575 automatic Fable baseline But probes**, giving529 task-native commands instead of1,104. Astra’s corresponding65-run total is446. This corrects analysis only; original results and grades are untouched. Full scenario/provider/arm scorecards are in [comparable-scorecards.md](comparable-scorecards.md), with all strata in CSV/JSON.

## Provider/study overview

Numbers below are But-only; each cell gives strict/state passes, mean seconds, and mean non-guide native calls. A zero guide-command count in the full-skill baseline does not mean its injected documentation was free.

| Study | Astra | Fable |
|---|---|---|
| Sep8 full | 65/65/65; 33.74s; 6.86 | 61/65/65; 24.22s; 8.14 |
| Sep9 stub | 39/39/39; 44.29s; 15.62 | 37/38/39; 28.76s; 10.05 |
| Sep10 control | 38/39/39; 37.73s; 16.36 | 37/39/39; 23.70s; 9.23 |
| i01 | 39/39/39; 42.23s; 11.05 | 37/37/39; 25.74s; 8.28 |
| i02 | 39/39/39; 38.84s; 11.00 | 38/38/39; 24.89s; 7.92 |
| i03 | 39/39/39; 37.29s; 13.05 | 38/39/39; 26.00s; 8.82 |
| i04 | 39/39/39; 37.97s; 12.59 | 37/37/39; 26.03s; 8.33 |
| i05 | 39/39/39; 36.26s; 11.33 | 36/37/39; 25.94s; 8.56 |
| i06 | 39/39/39; 35.82s; 11.44 | 39/39/39; 24.92s; 8.13 |

Sequential cohorts are not randomized evidence that a particular appended sentence caused a change. The full baseline additionally differs in product, top-level instructions, skill delivery and k=5 versus k=3. Prebaseline Fable-predecessor runs request `claude-opus-5` and are not silently relabeled Fable5.1.

## Fast paths are real and should survive a rewrite

The54 local [case capsules](selected-evidence-index.md) balance all19 canonical/partial strict failures with26 successful low-command i06 examples (every scenario/provider) and nine audit precedents. Low-command means lowest among three i06 repeats, not globally optimal or necessarily brief.

- Selective validation: both providers succeed with `but diff` then one commit of the selected IDs. Astra’s selected case uses2 non-guide calls and19.63s; Fable2 and14.64s. The outputs supply IDs without another status preflight.
- Multi-amend: both use `status -fv`, `diff`, and three amendments chained from those IDs;5 native calls each. Fable finishes immediately after the final amendment’s status output.
- Split commit: both use status, `uncommit && diff`, then three replacement commits and one move of the preserved top commit;7 calls. This shows the multi-step recipe carries real ordering information worth retaining.
- Reorder: Fable’s `status` plus one anchored block move succeeds with2 calls/14.53s. Astra’s lowest i06 example still uses30 calls, because it audits every old/new patch and message afterward.
- Squash: Fable chains two squash operations from one status and stops;3 calls/14.39s. Astra’s low example adds two tree-hash reads,5 calls/22.23s.
- Formatting cleanup: Astra uses diff then selected discard with status-after;2 calls/22.27s. Recovery likewise succeeds for both providers with verbose status plus one file-level uncommit;2 calls/19.08s Astra and16.71s Fable.
- Cold adoption shows `setup`, inspection and selective commit succeeding while preserving ignored files. Its extra setup/orientation is part of the different task state, not ritual overhead to eliminate everywhere.
- Scoped review is fundamentally analytical. Astra reads the two named committed JSON versions and compares IDs/types/fields; its3 native calls are useful task work. Fable’s selected case extracts the committed versions and answers with5 native calls, without invoking the guide. Lack of a guide call does not establish a failure.

[Fable reorder: exact status/move inputs and output](/Users/kiril/tmp/but-skill-evidence-2026-09-10/benchmark/selected-evidence/stub-guidance-20260910-i06--stub-claude-warm-004-pilot-4-reorder-commits-claude-butplusskill-r1.json); [Astra selected validation](/Users/kiril/tmp/but-skill-evidence-2026-09-10/benchmark/selected-evidence/stub-guidance-20260910-i06--stub-codex-warm-013-pilot-1-selective-validation-codex-butplusskill-r2.json); [Astra split recipe](/Users/kiril/tmp/but-skill-evidence-2026-09-10/benchmark/selected-evidence/stub-guidance-20260910-i06--stub-codex-warm-015-pilot-3-split-commit-codex-butplusskill-r2.json); [Astra required scoped-review comparison](/Users/kiril/tmp/but-skill-evidence-2026-09-10/benchmark/selected-evidence/stub-guidance-20260910-i06--stub-codex-warm-031-pilot-7-scoped-review-codex-butplusskill-r3.json).

## Extra checking differs more in depth than in presence

Across377 complete trials/provider, inspections occur after the final successful mutation in193 Astra trials and195 Fable trials. Those nearly equal counts do **not** imply equal work:127 Astra trials have Python-attributed Git subprocesses versus one Fable trial. Astra uses38 recorded failed `python` invocations before a retry/fallback, while Fable has none. These are environment/tool-selection errors, not model API outages.

The127 includes24 scoped-review cases whose structured comparison directly answers the user’s question. It is wrong to label all Python use waste. The remaining distribution spans selective copy26, integration24, reorder24, dirty update13, dual-source sync10 and recovery6. Scripts frequently expand one model tool call into many native Git reads; neither unit alone measures effort well.

Reorder is a sharp delivery/adherence distinction: Astra audits6 original/new message and patch pairs in all24 completed stub/candidate repetitions, versus0/5 full-skill baseline repetitions. Fable has no such Python Git reorder audit. The current reorder recipe says one anchored block move; Astra follows that mutation recipe. It does not explicitly forbid preservation audits, and the task asks for preserved content/order. Do not present the additional checks as defiance of a nonexistent blanket stopping rule.

Dual-source sync provides a counterweight against overclaiming novelty: its i06 rise43→64 native calls is entirely21 extra post-mutation Git reads, with But calls and mutations unchanged. One28-Git-call Python verifier drives the rise; it checks16 old/new metadata/patch outputs across four commits. The same four-commit audit occurred in1/5 full-skill baseline trials, was absent in i05, and returned in1/3 i06. Its native runtime is902ms versus30.81s for the complete post-mutation tail. Generation, roundtrips, provider time and final response account for the rest collectively; the evidence does not isolate them further.

Output sufficiency is narrower than “status proved everything”: final sync status already displays current main base, intended commits/authors/subjects, sibling branch and pending README. It does not independently prove exact prior patch bytes, README bytes, remote immutability or every operation-marker path. Those checks have real task referents. Conversely, i06 sync r3’s Git README diff followed by But diff repeats the same visible content before mutation. Its checksum is stronger preservation evidence and should not be conflated with that repeated display.

Integration also requires scope care. Once Astra has explicitly left But mode via teardown to main, Git status/index checks inspect the ordinary checkout required by the task. They are not the earlier recovery index mistake. Repeated status/stash checks and one `npm test` in i06 remain supplementary work, while exact preservation requirements still matter. The core has no dedicated full integration recipe.

[i06 sync, item_16 audit and item_15 prior output](/Users/kiril/tmp/but-skill-evidence-2026-09-10/benchmark/selected-evidence/stub-guidance-20260910-i06--stub-codex-warm-034-pilot-11-dual-source-sync-codex-butplusskill-r3.json); [full-skill sync precedent, item_13](/Users/kiril/tmp/but-skill-evidence-2026-09-10/benchmark/selected-evidence/baseline-20260908T104000Z--baseline-codex-warm-172-pilot-11-dual-source-sync-codex-butplusskill-r5.json); [integration teardown and follow-up audit](/Users/kiril/tmp/but-skill-evidence-2026-09-10/benchmark/selected-evidence/stub-guidance-20260910-i06--stub-codex-warm-023-pilot-12-integrate-topics-codex-butplusskill-r2.json).

## Real failures: distinguish task state, policy and guard limitations

There are18 strict failures in the754 complete But trials, plus one in partial i07. Only eight complete failures damage a recorded final-state condition; all are Fable selective-copy `unfinished_only`. With i07 that is9 state failures. Their other11 task checks pass, including destination tree, source/main/sibling refs, draft preservation, remote configuration/refs, no unfinished operation and independent functionality. The broad label `PROTECTED_HISTORY_DAMAGE` does not describe what actually failed.

All9 have independently confirmed Python bytecode residue: eight from retained tar members, one from the stopped completed i07 workspace. See [copy-residue-verification.json](/Users/kiril/tmp/but-skill-evidence-2026-09-10/benchmark/copy-residue-verification.json). The tests create `__pycache__`; some final checks occur before the test and therefore miss the side effect. In i05 r2 status does show the cache and the final response explicitly calls it harmless and delegates deletion to the user. The failure is responsibility for an observed side effect, not merely absence of a post-test check. i06’s successful Fable copy case removes its own cache after testing and passes. i07’s failed copy even removes its temporary archived checkout using a broad `/tmp/rwng.*` glob while leaving the workspace cache: a reminder to distinguish agent-created artifacts by scope, not add broad cleanup rules.

The10 complete protocol-failed trials all pass final state. Five Fable trials attempt `git fetch origin`; three Fable recovery trials attempt raw index reset/restore (four blocked command invocations across those three trials). These are actual raw Git mutations under a But-only write policy. Two other cases require separate treatment:

- Astra’s Sept10 control dirty-update case calls read-only `git reflog main -5`, which the frozen guard blocks. This is a guard/classification limitation, not evidence that reflog inherently mutates history.
- Fable’s Sept10 control sync r3 literally invokes `git -C workspace-origin.git` without a subcommand, then `git ls-remote origin`. Both raw input and native trace agree; there is no missing write subcommand to infer. The conservative guard rejects the malformed invocation. Preserve the canonical failure but do not label it a genuine write attempt.

The index failures have a concrete remedy boundary: But’s pending area and raw Git’s index are not equivalent. In recovery, a correct uncommit can leave the draft reported as staged by Git while satisfying the But task. But in sync the user explicitly requests the unfinished README to remain unstaged, so some index inspection is directly relevant. A blanket “never inspect the index” rule would erase this distinction.

[copy: cache observed and cleanup deferred](/Users/kiril/tmp/but-skill-evidence-2026-09-10/benchmark/selected-evidence/stub-guidance-20260910-i05--stub-claude-warm-024-pilot-13-selective-copy-claude-butplusskill-r2.json); [copy: test, scoped cleanup, pass](/Users/kiril/tmp/but-skill-evidence-2026-09-10/benchmark/selected-evidence/stub-guidance-20260910-i06--stub-claude-warm-036-pilot-13-selective-copy-claude-butplusskill-r3.json); [malformed Git invocation: toolu_014jjnWQWkzpAGJpgFFmwuVr](/Users/kiril/tmp/but-skill-evidence-2026-09-10/benchmark/selected-evidence/stub-skill-20260910--stub-claude-warm-034-pilot-11-dual-source-sync-claude-butplusskill-r3.json); [recovery raw-index correction attempts](/Users/kiril/tmp/but-skill-evidence-2026-09-10/benchmark/selected-evidence/baseline-20260908T104000Z--baseline-claude-warm-099-pilot-10-recover-amend-claude-butplusskill-r3.json).

## Command and documentation friction beyond those cases

The corrected complete But corpus contains21 failed `but resolve conflicts` queries after conflict-free states (15 Fable,6 Astra). Error excerpts explicitly say the branch has no conflicted commits. These are often checking an already resolved state, not failed resolution. The visible stopping condition should be precise about commit-versus-branch scope.

Seven `but teardown` invocations fail to infer a checkout target and request `--checkout-to`; four `but status` failures involve setup/mode transitions. These are boundary/orientation problems, distinct from ordinary dirty-file inspection. The local commands file includes exact output and source receipts. Twelve nonzero `but diff` outcomes include selector failures and bounded-output cases, so the family count alone is not twelve product defects. Two baseline Fable commits pass bare hunk suffixes and fail to resolve them; successful current fast paths use `<file>:<hunk>` IDs. Keeping short concrete ID grammar and task-appropriate first reads is supported by both failures and successes.

Astra issues12 reword mutation dispatches across the complete But corpus; Fable none. Representative dirty-update transcripts compare exact raw commit message bytes, encounter a newline difference, then call reword and receive “No changes to commit message - nothing to be done.” This is an observed additional repair attempt whose success exit does not imply a history change. The local capsules retain the assertion failure, raw inspection, attempted reword and no-op output.

Provider documentation behavior differs. Complete-corpus native guide calls are Astra297 bare,85 reference,15 full; Fable270 bare,89 reference,12 full and one concepts. No standalone examples call is observed. Full mode itself can include examples, and unused does not mean unneeded. Reference queries are often intentionally narrowed with grep; do not treat every pipe as accidental guide truncation.

The known Sept9 problem is narrower: all12 Fable full-guide calls are piped;10 specifically use head400. Large resulting outputs spill to saved files with short previews, sometimes obscuring task inspection bundled after documentation. Native full-guide status1 often reflects the pipe closing, not invalid syntax. Updated Sept10 bare-core delivery removed that particular truncation mechanism for observed loaders, but extra audits persisted. More core text alone is therefore not an evidenced fix.

[dirty-update newline/no-op sequence, items10–13](/Users/kiril/tmp/but-skill-evidence-2026-09-10/benchmark/selected-evidence/stub-skill-20260909--stub-codex-warm-018-pilot-6-update-dirty-branch-codex-butplusskill-r2.json); [full-guide spill, saved-file search, copy verification](/Users/kiril/tmp/but-skill-evidence-2026-09-10/benchmark/selected-evidence/stub-skill-20260909--stub-claude-warm-012-pilot-13-selective-copy-claude-butplusskill-r1.json).

## What this supports—and what it does not

For a main-skill rewrite, retain the successful compact inspection/ID/chaining recipes; make command-result guarantees and stopping scope explicit; distinguish But pending state from the raw index; describe completion across mode transitions; and clarify that supplementary validation must preserve the user’s workspace and clean its own artifacts. These are general behavior boundaries supported across scenarios, not proposed fixture-specific wording.

Do not ban useful verification or structured comparison. Do not infer all repeated commands are unnecessary, all status42 commands are actual writes, all nonzero statuses are defects, every uncalled section is expendable, or every unseen normalized output was absent from the model. Actual pipeline and Skill expansion behavior matters. The current exact-core receipt flag is limited to normalized tool results; separate prior-observer receipts cover Claude Skill content injected as user messages. Neither proves attention.

Provider-reported tokens/cost/API time are retained without inventing missing values. Cached-token accounting differs by provider; cached Astra input is already included in input, while Fable cache creation/read are separate fields. Warm/cold fixtures, prefix reuse, product/docs/top-level instructions, model versions and repetition count confound cross-cohort timing. Earlier token/cache and run-variation reports remain scoped to their original Sept8/9 evidence; this extraction does not claim a fresh causal cache analysis.

The machine-readable extraction, schema notes, all54 self-contained cases and exact source/hash manifest allow another agent to challenge these conclusions without re-running a benchmark. No benchmark inputs, loop controls, original results, repositories or credentials were changed.
