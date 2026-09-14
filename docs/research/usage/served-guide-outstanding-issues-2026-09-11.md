# Outstanding issues from the served-guide transcripts

The main remaining problem is **extra discovery and verification around otherwise successful operations**, with two concrete sync problems mixed in: incorrect fetch guidance and empty duplicate commits. More general instructions to be brief or stop checking are unlikely to address these mechanisms.

This is a read-only follow-up to the [12-run study](served-guide-2026-09-11.md). It uses a census of guide calls and command errors across all 624 September 11 trials, close reading of selected command sequences, actual command stdout, prompts and agent events, and comparison with historical baseline transcripts. The focused reviews cover 44 Astra trials across four scenarios and baseline/r01/r03/r12, all 26 final Fable command/message sequences with selected historical comparisons, and all 48 study plus 10 baseline sync sequences. These subsets overlap. No experiments were restarted and no skill or product changes were made. The report remains uncommitted.

## Where the remaining gap is

Final confirmation versus September 8 baseline, mean native VC calls per scenario. Guide loads and identified automatic probes are excluded. These are command counts, not a judgment that every extra call is unnecessary.

| Scenario | Astra baseline → confirmation | Fable baseline → confirmation |
|---|---:|---:|
| Dirty branch update | 6.2 → 15.5 | 15.0 → 21.0 |
| Integrate topics | 15.6 → 23.5 | 21.2 → 17.5 |
| Cleanup noise | 2.0 → 7.5 | 3.0 → 5.5 |
| Dual-source sync | 15.4 → 19.5 | 19.6 → 19.5 |
| Recover amend | 2.2 → 5.5 | 6.6 → 6.0 |

Dirty update, integration and cleanup account for **66.4% of Astra's gross positive call excess** across the thirteen scenarios. The denominator adds only positive scenario differences, before offsetting the selective-copy improvement. For Fable, dirty update is the largest call regression; split and cold adoption also regress in time without a comparable increase in native calls.

The type of extra work matters. Astra's Git reads rise from **2.40 to 4.46 per trial**, while mutation dispatches barely change (**2.05 → 2.08**). Fable's Git reads rise **3.68 → 4.46**, But reads **1.88 → 2.42**, and mutation dispatches **2.06 → 2.12**. Improving command selection and the evidence agents get back is a better target than shortening the mutation recipes indiscriminately.

## 1. Correct the fetch claim; it produces a visible wrong sequence

**Owner: skill accuracy, with CLI feedback contributing. Confidence: high.**

The retained guide says “Both commands fetch” about pull and branch update. Native fixtures from the study established that `but branch update` uses already-fetched tracking refs; it does not fetch. `but pull` fetches, and `but pull --check` can fetch without integrating the target.

In **18/24 Astra dual-source-sync trials**, the sequence is:

```text
but branch update feature-service
but pull
… discover the remote feature change is still missing …
but branch update feature-service
```

Both final confirmation trials do this. The first update prints `Updated branch feature-service.` even though the new remote change has not been fetched. The agent then inspects status/history, consults update help, and updates again. The short success message does not explain which remote state was used.

The two factual-correction candidates, runs 4 and 8, and the recipe-specific pull-first candidate, run 10, account for the other **6/24 Astra trials**: all fetch first and use one actual branch update. This is a repeated, concrete behavioral association with the intended copy, although the adaptive experiment does not establish an isolated overall speed effect. Fable also repeats the false fetch claim in its final run-12 sync-r2 answer despite having used the correct order.

**Next action:** fix the fact independently of the efficiency gate. Our previous keep/reject rule was too blunt here: a noisy, slower k=2 matrix is not a sound reason to retain a known false statement. Separately, consider making the CLI say which tracking ref/tip it integrated and that it did not fetch. Do not describe historical tracking-ref inspection as a guaranteed fetch operation.

## 2. Branch reconciliation creates extra work of its own

**Owner: CLI behavior and output. Confidence: high for the observed fixture; intended general empty-commit policy needs review.**

In **44/48 September 11 sync trials**—all 24 Astra and 20 Fable—the inspected output explicitly shows an empty duplicate of the shared feature commit, and the agent subsequently removes it. All four final confirmation sync trials have this detour.

After branch update, the graph contains both:

```text
owm#0  Support document format  (no changes)
owm#1  Support document format  [contains the shared file change]
```

Agents have to establish that the first is safe to remove, discover whether `uncommit` or `discard` is appropriate, remove it, and inspect the result. In final Fable sync r1/r2, this alone prompts three reference invocations across the two trials. An instruction to trust success cannot eliminate repository state that still needs attention.

This is **not a newly proven regression in the binary**: four of five historical Fable baseline sync trials also use branch update and discard the duplicate. All five Astra baseline trials instead use `but pick` for the fetched environment commit. The new guide steers Astra toward a path with this extra cleanup requirement.

**Next action:** investigate why this reconciliation retains the duplicate and whether it should be dropped or explicitly reported with a safe resolution. Do not broadly tell agents to delete empty commits; intentional empty commits are a different case. If product behavior must remain, the relevant recipe needs a narrow explanation of this observed case.

## 3. Preservation requirements turn into repeated audits

**Owner: instruction/operation contract and evidence selection. Confidence: high for the sequences; savings require testing.**

The task prompts explicitly require retaining unrelated work, authors, messages, and sometimes exact file bytes. Some verification is justified. The problem is the accumulation of overlapping views before and after the same operation.

Examples from final Astra:

- **Dirty update, both trials:** initial But/Git status, refs/log/remotes and file snapshots precede pull; further refs/log/status checks follow successful resolution. All five historical Astra baseline trials start directly with `but pull`.
- **Cleanup, both trials:** protected-ref/context snapshots and final inspection surround discard. All five Astra baseline cleanup trials use exactly `but diff` then `but discard`.
- **Integration r1:** after status/log/stash inspection, a final Python audit adds **15 native Git calls**, including nine `git show` dispatches to compare metadata. Some of this rechecks what the successful operation already reported; preservation of explicitly named file contents remains a legitimate separate question.
- **Recovery:** the retained pending-work clarification removed discard/recreate, but help, author and protected-reference inspections sometimes remain. The two final Astra trials use **2 versus 9 calls** for the same successful operation. This is a smaller unresolved tail of an otherwise successful fix.

Fable exhibits the same tendency less uniformly. Final cleanup r1 uses eight calls while r2 uses three with the same guide. Split r2 adds three reads relative to the concise route followed by r1: a dirty diff before uncommit, a committed diff, and another final diff. Both final dirty-update trials inspect broadly before pull; one adds an unrequested `pull --check` preview.

**Next action:** identify the exact invariant an operation guarantees and the one task-required fact its output does not establish. Prefer narrow, accurate contracts at the relevant operation over another general “do less verification” sentence. The selection-preservation and scoped-uncommit candidates produced promising local behavior, but did not meet the full-matrix acceptance gate; treat them as hypotheses, not retained wins.

## 4. Agents still lack a clear route to some necessary facts and syntax

**Owner: CLI discoverability/reference coverage first, skill wording second.**

The integration task requires leaving the workspace for a plain checkout and explicitly prohibits pushing. The frozen merge help distinguishes a local `gb-local` target from a real remote target that merge would push to. Establishing which target is configured therefore matters before execution. These are genuine facts to discover, not automatically unnecessary verification.

- **Astra final integration r2:** reads status JSON, then `.git/gitbutler/virtual_branches.toml` and `.git/config` to discover `targetRef = refs/remotes/gb-local/main`. This suggests the normal response is not making the configured target easy enough to identify. It does not prove no public command can provide it.
- **Fable final integration r2:** searches `but skill reference` for a teardown section, gets no match, then calls `but teardown --help`. The frozen `inputs/served-reference.md` has **zero occurrences of `teardown`**; this is a confirmed omission, not merely an unsuccessful heading search. The guide's plain-checkout path needs a discoverable syntax reference.
- **Across all runs:** Fable invokes bare teardown and gets the required-`--checkout-to` error in **9/24 integration trials**. Adding the exact syntax in run 7 did not eliminate help use, so simply repeating syntax is not a demonstrated solution.
- **Astra final integration r1:** attempts to inspect branch refs after merge output explicitly reports those branches deleted. It also hits a Git revision/path ambiguity. These failures create more investigation after a successful operation.
- **Fable final sync r1:** guesses `refs/gitbutler/feature-service`, gets an unknown-revision error, then succeeds with `feature-service`.

**Next action:** make the configured target and checkout destination easy to retrieve, ensure reference coverage includes the guide's advertised paths, and make deleted/rewritten refs clear in operation output. Teach a targeted lookup route, not internal-file inspection or guessed namespaces.

Help use remains substantial: Astra makes **171 CLI-help dispatches across 106/312 trials**; Fable **66 across 46/312**. The final repeat has 14 Astra help calls and three Fable calls. Some answer missing syntax or important operation semantics; these totals are not all avoidable overhead. Expanding the main guide with every flag is not a demonstrated remedy.

## 5. Guide loading is mostly reliable; mutation skips are the actionable minority

**Owner: stub/loader behavior. Confidence: census of these runs.**

| Provider | Trials with served guide loaded | Trials without |
|---|---:|---:|
| Astra | 312/312 | 0 |
| Fable | 285/312 | 27 |

**24 of Fable's 27 skips are the read-only scoped-review scenario, and all pass.** The three mutation skips are:

- Run 4 recovery r2: greps the stub, attempts blocked `git restore --staged`, and fails the protocol check while passing the repository-state checks.
- Run 9 recovery r1: greps the stub for uncommit documentation that is not there; eventually passes with extra discovery.
- Run 11 cleanup r1: skips loading and passes.

Fable loads the guide in **all 24 final-confirmation mutation trials**. Therefore, skipped loading does not explain its final mutation regressions. It remains a real intermittent failure mode that changes to the served content alone cannot fix.

There are **no `--full` or examples calls anywhere in these 624 trials**. Reference requests occur, especially around integration and sync, but the old full-guide-loading problem is not the current explanation.

**Next action:** if testing loader wording later, evaluate mutation discovery separately. Do not optimize for making every successful read-only review pay for loading the full guide.

## Lower-priority issues that should not become more skill prose

- **Test-runner discovery:** Fable final selective-copy r2 guesses pytest (unavailable), masks the fallback through a pipeline, runs unittest with zero discovered tests, then reads and runs `python3 test_commands.py`. The other trial uses the correct runner directly. This is repository/test-runner discoverability and shell handling, not a GitButler preservation problem.
- **Cleanup timing:** the run-7 Fable selective-copy failure checks status before Python tests, then leaves their generated bytecode cache. The guide already says to check after the last own command. This is an observed execution failure, not evidence that the instruction is absent.
- **Timing noise:** identical native/model-call counts sometimes have very different durations. Final Fable split r1 uses seven VC calls and takes 38.568s; r2 uses ten and takes 23.835s. The run-9 timeout follows completed setup help and has no identified provider error; it remains unexplained. Neither phenomenon supports attributing a fixed number of seconds to hidden reasoning or to guide length.

## What to prioritize

1. **Correct the false fetch statement.** This is accuracy work, not a speed experiment.
2. **Investigate empty duplicate handling and branch-update feedback.** This creates a recurring task beyond the requested reconciliation.
3. **Fix discoverability of target/checkout and missing reference routes.** Agents currently guess or inspect internals.
4. **Then test one narrow preservation contract at a time**, starting with cleanup and dirty update, where the remaining call excess is concentrated. Retain task-required evidence; remove duplication only where the operation's real contract supports doing so.
5. **Treat loader reliability as a separate experiment.** It is not the dominant final-run issue.

Do not discard the demonstrated gains: reorder and squash have concise command paths, selective copy improves substantially for Astra, pending/index clarification removes pointless file recreation, and the final matrix passes all 52 checks. The outstanding work is concentrated, not a reason to rewrite every section again.

## Evidence and interpretation limits

Historical baseline uses k=5, final confirmation k=2. Product/reference/instruction versions differ. All-run frequencies combine adaptive candidate versions and repeated fixtures; they describe these trials rather than independent real-world prevalence. Command counts exclude ordinary shell/file/test work and can hide significant effort in one Python or shell call. A read may be redundant, required, or exploratory; counts alone cannot decide which.

Actual native stdout is authoritative for claims about CLI output. Some normalized `agent-events.json` tool results retain only the final output chunk; an omitted success line there does not show that the CLI failed to print it.

Raw trial roots on butgym:

```text
/home/hermes/.local/state/butgym/served-guide-20260911-rNN/matrix/{codex,claude}-{warm,cold}/<trial>/
/home/hermes/.local/state/butgym/baseline-20260908T104000Z/matrix/<batch>/<trial>/
```

Each contains `prompt.txt`, `agent-events.json`, `trace-buckets.json`, `result.json`, and native `command-output/<command-id>.stdout` / `.stderr`. Final warm trial suffixes useful for the cases above:

| Case | Repetition 1 | Repetition 2 |
|---|---|---|
| Dirty update | `006-pilot-6-update-dirty-branch` | `018-pilot-6-update-dirty-branch` |
| Cleanup | `008-pilot-9-cleanup-noise` | `020-pilot-9-cleanup-noise` |
| Recovery | `009-pilot-10-recover-amend` | `021-pilot-10-recover-amend` |
| Sync | `010-pilot-11-dual-source-sync` | `022-pilot-11-dual-source-sync` |
| Integration | `011-pilot-12-integrate-topics` | `023-pilot-12-integrate-topics` |
| Selective copy | `012-pilot-13-selective-copy` | `024-pilot-13-selective-copy` |

Full names are `stub-<provider>-warm-<suffix>-<provider>-butplusskill-r<repetition>`. For example, final Astra sync r2 is `stub-codex-warm-022-pilot-11-dual-source-sync-codex-butplusskill-r2`. Its first misleadingly terse update output is command `424daebf-ddbd-48dc-9e5b-e15ff89c1807`; the later graph exposing the duplicate is `1420df3a-ea1e-43c4-9362-43248592fd4d`.

Private local supporting material, kept outside the repository:

- [Call-gap arithmetic](/Users/kiril/.local/state/butgym/served-guide-loop-20260911/outstanding-quantitative.json).
- [Sync sequences and selected actual output: 48 study + 10 baseline trials](/Users/kiril/.local/state/butgym/served-guide-loop-20260911/sync-transcript-extract.json).
- [Guide/error census: 624 trials](/Users/kiril/.local/state/butgym/served-guide-loop-20260911/loading-outstanding-evidence.json).
- Detailed reviewed cases: [Astra](/Users/kiril/.local/state/butgym/served-guide-loop-20260911/astra-outstanding-review.md), [Fable](/Users/kiril/.local/state/butgym/served-guide-loop-20260911/fable-outstanding-review.md), [loading and errors](/Users/kiril/.local/state/butgym/served-guide-loop-20260911/loading-outstanding-review.md).
