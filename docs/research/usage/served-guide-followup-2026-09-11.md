# Served guide follow-up — September 11

**One requested k=2 run complete: 52 canonical trials, 51 strict passes, all 52 repository-state checks passing. Baseline parity is not reached.** Astra uses fewer native VC calls than the previous confirmation, while Fable regresses mainly around plain-checkout discovery. No optimization candidates or additional full runs were launched.

Tested the source tree of `origin/served-skill-guide` at `b8bbcc91f988ca14c8d4052f727309ffc5880be9` on butgym, using its newly shipped stub and the content served by `but skill`. Both GPT 6 Astra and Fable 5.1 ran at medium effort, all 13 But scenarios, two repetitions. The providers ran concurrently with one worker each. Matrix elapsed time was about 14m40s, excluding build and preflight.

## Overall comparison

Means per trial. VC calls exclude guide loading and identified automatic probes. They include native Git dispatches from Python and differ from model tool calls. Previous means the unchanged run-12 confirmation, not the best selected run-3 estimate.

| Model / version | Strict passes | State passes | Seconds | VC calls | Model tool calls |
|---|---:|---:|---:|---:|---:|
| Astra: Sept 8 full skill | 65/65 | 65/65 | 33.74 | 6.86 | 6.85 |
| Astra: Previous confirmation | 26/26 | 26/26 | 32.05 | 9.04 | 7.31 |
| Astra: Latest branch | 26/26 | 26/26 | 32.91 | 8.12 | 7.50 |
| Fable 5.1: Sept 8 full skill | 61/65 | 65/65 | 24.22 | 8.14 | 4.62 |
| Fable 5.1: Previous confirmation | 26/26 | 26/26 | 24.25 | 9.12 | 5.27 |
| Fable 5.1: Latest branch | 25/26 | 26/26 | 26.50 | 9.58 | 5.62 |

- **Astra versus previous:** VC calls −10.2%, elapsed time +2.7%, model tool calls +2.6%. Against the full-skill baseline: time −2.5%, VC calls +18.3%. Its command gap has narrowed from +31.7% in the previous confirmation.
- **Fable versus previous:** VC calls +5.1%, elapsed time +9.3%, model tool calls +6.6%. Against baseline: time +9.5%, VC calls +17.7%.
- Guide-inclusive VC means are Astra 10.04 → 9.12 and Fable 10.23 → 10.85. The historical full-skill installation did not require equivalent served-guide commands; keep this distinction when interpreting effort.

## What changed in behavior

### Fetch order improved in every sync trial

All four new sync trials fetch before the first actual branch update, versus two of four previously. Actual branch-update executions fall **6 → 4** across the four trials. Fable adds one dry-run preview; that is separate from the four real updates. All four sync trials pass.

Astra sync improves from **51.38s / 19.5 calls → 45.38s / 16.5 calls**. Fable remains pull-first but does not gain overall in this scenario: **33.93s / 19.5 → 37.53s / 20.0**. The wrong-order detour is gone; that does not imply every provider must become faster.

An empty duplicate shared commit still appears and is removed in **4/4 trials**, as in the previous confirmation. The source update did not fix the underlying branch-reconciliation behavior.

### Target discovery improved; checkout discovery remains the main problem

All four integration trials use **`but config target`**, versus zero previously. The command identifies `gb-local/main`, so agents can establish that the operation targets local main. Both Astra integration trials find teardown through top-level help and complete the requested checkout. Their mean improves from **57.36s / 23.5 calls → 39.21s / 15.0 calls**, also beating the historical baseline for this scenario.

Fable’s two integration transcripts expose the unresolved exit path:

1. **Repetition 1:** loads the guide and successfully identifies the target and merges. It searches the reference, branch help and concepts for a checkout route, then tries blocked `git checkout main`. Only afterward does top-level help reveal teardown. It completes `but teardown --checkout-to main`. All 11 repository-state checks pass, but the prohibited Git attempt is retained as a strict protocol failure. Time: **69.799s**, versus 27.363s previously.
2. **Repetition 2:** avoids the prohibited Git command, but never discovers or runs teardown. It searches the reference four times, leaves `gitbutler/workspace` checked out, and incorrectly claims switching to main requires forbidden raw Git. The verifier accepts the clean workspace as equivalent to main (`but_workspace_retained: true`, `active_main: true`), so this trial passes. Time: **51.307s**, versus 37.416s previously.

**Do not interpret the second passing grade as successful checkout discovery.** The task asks for a clean checkout of integrated main; this result exposes the distinction between the verifier’s accepted equivalent workspace and actually checking out main. No grading rules or historical scores were changed. If direct checkout is a required benchmark property, specify and enforce that in a separate harness change.

The current guide removed the teardown command name, while the served reference still omits it. The reference/config-target addition solves one discovery problem but not the exit route. The sequence is consistent with that missing route; k=2 and multiple simultaneous input changes do not isolate removal of one sentence as the sole cause.

Fable integration rises from **32.39s → 60.55s**, accounting for **95.9% of its net increase in scenario-mean time** across the suite. This is descriptive decomposition, not an estimate of seconds causally attributable to one wording change.

### Some narrower gains; verification remains variable

- Fable dirty-update calls decrease **21 → 16.5**, with time essentially unchanged at 33.94s. Broad preinspection/preview is reduced in the reviewed traces; explicit WIP preservation checks remain relevant.
- Fable cleanup improves **24.97s / 5.5 calls → 17.25s / 5.0 calls**.
- Astra cleanup calls decrease **7.5 → 6.0**, but time increases **28.68s → 30.16s**. One repetition still performs paired ref/context audits.
- Astra dirty update remains at **15.5 calls**, with time **41.37s → 43.72s**. The broad preservation-inspection habit remains.
- Reorder and squash retain their concise two- and three-call paths for both providers.

### The new stub does not show a loading gain in this small comparison

Astra loads the complete guide in **26/26** trials; Fable **24/26**, with both omissions being read-only scoped reviews. Every mutation trial loads the guide. These counts match the previous confirmation; this small sample does not establish a change in the rare mutation-skip rate. Native and normalized receipts contain the complete frozen guide wherever loaded.

There are no `--full` or examples calls. Fable reference calls increase **5 → 8**, plus one concepts call. Six of its eight reference calls are in integration. Astra uses no reference calls. Eight Astra trials have initial VC reads: six warm trials issue But inspections before loading, contrary to the stub’s “before your first `but`” instruction; two cold trials issue only read-only Git commands, which are allowed. No writes precede guide loading. This loading-order observation is separate from the benchmark’s strict protocol grade.

## Per-scenario comparisons

Each latest scenario has two trials. All state checks pass. The only strict failure is Fable integration repetition 1; repetition 2 has the equivalent-workspace caveat above. Baseline k=5; previous/latest k=2.

### Astra

| Scenario | Baseline seconds | Previous seconds | Latest seconds | Baseline calls | Previous calls | Latest calls |
|---|---:|---:|---:|---:|---:|---:|
| Selective validation | 23.46 | 21.49 | 23.78 | 2.00 | 2.50 | 3.00 |
| Multi-amend | 25.84 | 24.74 | 33.41 | 5.00 | 5.50 | 6.00 |
| Split commit | 28.03 | 27.55 | 29.84 | 7.00 | 7.00 | 7.00 |
| Reorder commits | 23.20 | 21.44 | 23.49 | 2.00 | 2.00 | 2.00 |
| Squash commits | 24.49 | 22.34 | 24.43 | 3.00 | 3.00 | 3.00 |
| Dirty branch update | 39.14 | 41.37 | 43.72 | 6.20 | 15.50 | 15.50 |
| Scoped review | 23.15 | 26.31 | 29.61 | 2.20 | 3.00 | 3.00 |
| Cold checkpoint | 32.39 | 35.80 | 35.98 | 8.20 | 10.50 | 10.50 |
| Cleanup noise | 24.09 | 28.68 | 30.16 | 2.00 | 7.50 | 6.00 |
| Recover amend | 26.43 | 24.02 | 28.62 | 2.20 | 5.50 | 6.00 |
| Dual-source sync | 50.93 | 51.38 | 45.38 | 15.40 | 19.50 | 16.50 |
| Integrate topics | 56.15 | 57.36 | 39.21 | 15.60 | 23.50 | 15.00 |
| Selective copy | 61.27 | 34.25 | 40.18 | 18.40 | 12.50 | 12.00 |

### Fable 5.1

| Scenario | Baseline seconds | Previous seconds | Latest seconds | Baseline calls | Previous calls | Latest calls |
|---|---:|---:|---:|---:|---:|---:|
| Selective validation | 17.52 | 14.25 | 15.41 | 2.40 | 3.00 | 3.00 |
| Multi-amend | 16.39 | 16.32 | 16.83 | 5.00 | 5.50 | 5.00 |
| Split commit | 19.57 | 31.20 | 25.69 | 7.00 | 8.50 | 8.50 |
| Reorder commits | 12.40 | 12.94 | 13.42 | 2.00 | 2.00 | 2.00 |
| Squash commits | 15.27 | 13.67 | 13.41 | 3.00 | 3.00 | 3.00 |
| Dirty branch update | 31.98 | 33.94 | 33.94 | 15.00 | 21.00 | 16.50 |
| Scoped review | 22.47 | 20.09 | 22.72 | 3.20 | 6.50 | 8.00 |
| Cold checkpoint | 22.62 | 31.30 | 29.56 | 7.80 | 8.00 | 9.50 |
| Cleanup noise | 16.22 | 24.97 | 17.25 | 3.00 | 5.50 | 5.00 |
| Recover amend | 24.02 | 19.83 | 25.06 | 6.60 | 6.00 | 9.00 |
| Dual-source sync | 42.52 | 33.93 | 37.53 | 19.60 | 19.50 | 20.00 |
| Integrate topics | 44.75 | 32.39 | 60.55 | 21.20 | 17.50 | 20.50 |
| Selective copy | 29.08 | 30.38 | 33.20 | 10.00 | 12.50 | 14.50 |

## What to do next

Keep the truthful fetch guidance and `but config target` route: both produce the intended observed behavior. The clearest next fix is to make the plain-checkout command discoverable in the reference and applicable recipe. Removing its name did not remove the task’s need for that operation. The duplicate-commit issue and redundant preservation checks remain separate work.

This follow-up does not meet the earlier all-pass/no-regression criterion, and it does not meet the baseline on overall time and calls for both models. It does not justify reverting accurate facts based only on aggregate timing. No edits, reversions, commits or pushes were made to the newly supplied skill during this test.

## Reproducibility and operations

- Source on VM: `/home/hermes/src/gitbutler-served-skill-20260911`. Frozen run: `/home/hermes/.local/state/butgym/served-guide-20260911-followup-01`.
- Candidate source tree `074ae533d289740a0d82e69c6113cb0243c0c1dd` exactly matches the remote branch. GitButler workspace commit is `2e09744093ba6a3f77e4d755f0c8de6214ae1e34`; its tree equality was independently verified. Partial-clone missing blobs were hydrated while updating the checkout.
- Built binary SHA-256: `d91a73b112805cee243c45f66b395a6cba4f3ee434aa5fe79690d38c7c7ba4c2`. Guide: `71de64ec75b0a7975c6330c16e4cd016864aa494db31c6f4f291dbe3262c4aa8`. Stub: `0b03103e92d93d1e54e4a15678a4b1539be85b38b48f30c8884b879681f88e4c`.
- Independent audit: all 189 harness hashes and non-But executable packages match the previous confirmation; models, medium effort, 180s agent timeout, 300s trial timeout, scenario set, k and grading remain fixed. Guide, stub and reference change together, plus the new source revision; this is not an isolated one-sentence experiment.
- An inherited `unchanged_confirmation` provenance block was stale. The original launched file is preserved and a separate `provenance-clarification.json` identifies the correct new candidate and executable records. Execution/finalization never reads the stale block.
- Preflight: Astra passed; Claude first failed because its OAuth token had expired. Normal Claude CLI refresh succeeded, then its preflight retry passed. The failed attempt and refresh receipt are preserved separately. There were **three benchmark preflight attempts (two passed, one failed)**, excluded from canonical scores. No canonical trial was retried.
- All 52 canonical results were verified; owned fixtures and the failed authentication fixture were archived with file hashes, types, modes and links checked before cleanup. The service is stopped (systemd records a failed exit because one trial failed). About **34.5 GiB free** remains; Cargo caches and frozen evidence are retained. The completed 12-run optimization ledger is unchanged.

Provider-native token/cost categories are in the linked token report, with reporting denominators. Fable reports mean cost $0.4484/trial; Astra cost is unreported, not zero. Categories are not interchangeable between providers, and reported thinking counters do not measure all reasoning.

## Evidence

[Previous results](served-guide-2026-09-11.md) · [Prior outstanding issues](served-guide-outstanding-issues-2026-09-11.md)

- [Full machine-readable comparison](/Users/kiril/.local/state/butgym/served-guide-loop-20260911/followup-01/served-guide-comparison.json).
- [Input/change audit](/Users/kiril/.local/state/butgym/served-guide-loop-20260911/followup-input-review.md).
- [Transcript case review and final behavior census](/Users/kiril/.local/state/butgym/served-guide-loop-20260911/followup-first-repeat-findings.md).
- [Guide delivery](/Users/kiril/.local/state/butgym/served-guide-loop-20260911/followup-01/guide-delivery.json), [validation](/Users/kiril/.local/state/butgym/served-guide-loop-20260911/followup-01/final-validation-agent.json), [cleanup receipt](/Users/kiril/.local/state/butgym/served-guide-loop-20260911/followup-01/cleanup-receipt.json), [token report](/Users/kiril/.local/state/butgym/served-guide-loop-20260911/followup-01/token-report.json).

Full transcripts and archives remain private on butgym. This report and the reading-pack index remain uncommitted.
