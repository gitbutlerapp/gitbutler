# Served-guide experiments — September 11

**Complete — 12/12 full runs; baseline parity not reached.** This tests the committed `served-skill-guide` branch through the shipped stub, with changes limited to the content printed by `but skill`. The original full-install skill, stub and generated reference stay fixed.

Each full run covers all 13 But scenarios with Astra and Fable 5.1, medium effort, two repetitions: 52 trials. Providers run concurrently, with one worker per provider. The limit is 12 full runs, including control and confirmations. Two authentication canaries per run are recorded separately.

## Outcome and useful conclusions

The exact-binary confirmation passes **52/52 strict and state checks**. Against September 8, Astra is **5.0% faster but uses 31.7% more non-guide VC calls**; Fable has essentially equal time (**+0.13%**) and **12.0% more calls**. The original selected point estimates do not reproduce exactly. The confirmation still beats the retained change's original parent (run 2) by **6.50% on the declared composite**, with every metric inside the regression guard; that comparison does not imply baseline parity.

Keep only `75cd24739a369d8f04d05c21a8afa7e34d1d8d29` (applied workspace context) and `d7ca1bc908a6328d9a5d28a76e0a29086bd607a8` (pending/index semantics). On butgym they are on `served-guide-tuning-20260911` in `/home/hermes/src/gitbutler-served-skill-20260911`. The source is clean and rebuilt. The served guide grew from **2,482 to 2,510 words: +28 net words**; the stub did not change. No push or PR was made.

- **Concrete state semantics were the strongest retained change.** The confirmation's four recovery trials leave the draft pending without discarding and recreating it to alter Git's index display. The workspace clarification also retains the distinction between applied branches and an explicitly requested plain checkout.
- **Exact operation contracts can help locally.** Selective-preservation and scoped-uncommit wording coincided with fewer Astra checks in their target cases, but neither candidate passed the full-matrix gate. Keep these observations separate from proven overall improvements.
- **More syntax or another “first” instruction was not enough.** Agents often kept orientation and verification reads after the checkout-syntax, pull-first and diff-comment edits. Requested preservation checks are not automatically waste.
- **A served guide cannot guide a trial that never reads it.** Some Fable recovery/cleanup trials skipped the guide and did substantially more discovery. That is a loader-reliability issue outside this fixed-stub comparison.
- **Correct factual errors independently of performance selection.** Native fixtures show branch update does not fetch. Both accurate fetch candidates were rejected by the declared experimental gates; their patches remain available. The retained source still has the false claim and needs that correction before publication.

Across all 12 matrices: **624 canonical trials, 620 strict passes and 621 state passes**. Another 24 passing canaries are excluded. All run services are stopped; temporary fixtures were archived and cleaned. The run-9 timeout remains a failed execution with a separately audited failure archive. About **35.4 GiB is free**, with **2.0 GiB of Cargo build output and 1.1 GiB of Cargo cache** retained.

## Results

Means are per trial. “VC calls” counts non-guide native But/Git command dispatches, including Python-invoked Git. It excludes guide loading, identified automatic probes, ordinary shell/file/test commands, and is distinct from model tool calls. Strict passes include protocol checks; state passes check the requested repository result.

| Version | Provider | Strict / state | Seconds | VC calls |
|---|---|---:|---:|---:|
| Sept 8 full skill, k=5 | Astra | 65/65 / 65/65 | 33.74 | 6.86 |
| Sept 8 full skill, k=5 | Fable | 61/65 / 65/65 | 24.22 | 8.14 |
| Sept 10 i06 stub, k=3 | Astra | 39/39 / 39/39 | 35.82 | 11.44 |
| Sept 10 i06 stub, k=3 | Fable | 39/39 / 39/39 | 24.92 | 8.13 |
| Sept 11 untouched served guide, k=2 | Astra | 25/26 / 25/26 | 31.79 | 9.04 |
| Sept 11 untouched served guide, k=2 | Fable | 26/26 / 26/26 | 27.13 | 9.15 |
| Run 2: workspace context | Astra | 26/26 / 26/26 | 32.67 | 10.00 |
| Run 2: workspace context | Fable | 26/26 / 26/26 | 25.65 | 10.00 |
| Run 3: pending-work semantics | Astra | 26/26 / 26/26 | 29.26 | 8.12 |
| Run 3: pending-work semantics | Fable | 26/26 / 26/26 | 26.15 | 9.04 |
| Run 4: fetch semantics, rejected | Astra | 26/26 / 26/26 | 28.84 | 8.38 |
| Run 4: fetch semantics, rejected | Fable | 25/26 / 26/26 | 27.07 | 10.19 |
| Run 5: direct pull/setup, rejected | Astra | 26/26 / 26/26 | 29.08 | 8.50 |
| Run 5: direct pull/setup, rejected | Fable | 26/26 / 26/26 | 25.55 | 9.04 |
| Run 6: selected preservation, rejected | Astra | 26/26 / 26/26 | 29.82 | 8.19 |
| Run 6: selected preservation, rejected | Fable | 26/26 / 26/26 | 25.86 | 9.31 |
| Run 7: checkout target, rejected | Astra | 26/26 / 26/26 | 31.10 | 8.96 |
| Run 7: checkout target, rejected | Fable | 25/26 / 25/26 | 23.23 | 9.19 |
| Run 8: minimal fetch correction, rejected | Astra | 26/26 / 26/26 | 31.07 | 8.31 |
| Run 8: minimal fetch correction, rejected | Fable | 26/26 / 26/26 | 25.42 | 9.69 |
| Run 9: scoped uncommit, rejected | Astra | 25/26 / 25/26 | 37.61 | 8.23 |
| Run 9: scoped uncommit, rejected | Fable | 26/26 / 26/26 | 24.59 | 9.62 |
| Run 10: pull-first recipe, rejected | Astra | 26/26 / 26/26 | 32.47 | 8.15 |
| Run 10: pull-first recipe, rejected | Fable | 26/26 / 26/26 | 22.58 | 8.73 |
| Run 11: diff-ID comment, rejected | Astra | 26/26 / 26/26 | 35.41 | 10.46 |
| Run 11: diff-ID comment, rejected | Fable | 26/26 / 26/26 | 24.18 | 9.62 |
| Run 12: unchanged run-3 confirmation | Astra | 26/26 / 26/26 | 32.05 | 9.04 |
| Run 12: unchanged run-3 confirmation | Fable | 26/26 / 26/26 | 24.25 | 9.12 |

In the untouched served-guide control (run 1), Astra ran substantially faster than in i06, especially in reordering, selective copy and integration. The final confirmation has a different distribution: its integration mean is 57.36s versus i06’s 58.89s, only 2.6% faster. This comparison also changes product/reference/instruction versions, so it does not isolate the guide's effect. It has not reached baseline parity: command counts remain higher, Fable takes longer, and the untouched control had one Astra sync failure. Retained run 3 passes all 52 trials.

## Observations from the control

- **Workspace context:** Astra reconciled both remote sources successfully, then ran `but teardown --checkout-to feature-service`. This removed an applied sibling's file. Recreating it as untracked broke workspace availability, the allowed unfinished-file set and active context. The broad recorded failure label is `PROTECTED_HISTORY_DAMAGE`, but the history checks themselves passed. Other trials retained the workspace and passed.
- **Pending work versus Git's index:** both Astra recovery trials successfully uncommitted the draft, then backed it up, discarded it and recreated it solely to change Git's index display. But's pending state was the same before and after. Fable also spent time investigating the index. Both providers had received the exact served guide.
- **Fetching:** the guide says both pull and branch update fetch. A separate native scratch experiment disproves that for this binary: branch update left a stale tracking ref unchanged; pull fetched it; branch update then integrated it. Pull-first avoids a repeated update, but does not by itself prevent the teardown failure or the empty duplicate commit observed in sync.
- **Reordering:** both providers now use one status and one move. Astra's scenario mean falls from i06's 45.64s and 32.67 commands to 20.13s and two commands. The previous extensive reconstruction is absent.
- **Dirty update:** all four trials handle the conflict loop correctly and stop querying once its output says resolution is complete. Extra work comes from orientation before pull, backup copies and preservation checks. Some checks answer explicit task requirements; they should not all be classified as waste.
- **Cold adoption:** Astra is faster despite more commands. One Fable trial deletes an empty branch created by setup and adds verification reads; another leaves it and passes. There is no evidence that the guide's cleanup sentence caused that deletion.

The whole frozen guide appears in recorded tool results in 26/26 Astra and 24/26 Fable trials. Fable's two scoped reviews omitted loading it. Six Astra trials inspected the repository before loading. Delivery proves content was available, not that the model attended to it.

## Candidate decisions

**Run 2 — keep for correctness.** Commit `75cd24739a369d8f04d05c21a8afa7e34d1d8d29` adds a short opening clarification: applied branches share the workspace; being on one means keeping it applied; teardown is for an explicitly requested plain checkout. All four sync trials retain the workspace and all four integration trials perform the requested checkout. The four-metric efficiency composite worsens by about 4%, so this is a documented correctness exception, not an efficiency improvement. k=2 does not establish causality. Both Astra recovery trials still discard and recreate the already-pending draft.

**Run 3 — keep for efficiency.** Commit `d7ca1bc908a6328d9a5d28a76e0a29086bd607a8` replaces the 69-word index paragraph with 67 words defining pending work and commit selection. It explicitly maps unstaged/uncommitted requests in a But workspace to pending state, explains why discard/recreate does not change that state, and retains the plain-checkout exception. Native checks support the behavior; Fable 5.1 reviewed the wording. All 52 trials pass; the four-ratio composite improves 9.54% versus run 2, with no metric beyond the 5% regression guard. Both Astra recovery trials avoid discard/recreate, averaging 20.76s versus 45.18s. The workspace clarification stays in place.

**Run 4 — reject under the declared gate.** Commit `0dfb9993745f8f309e57c4fd523559ac3d34362b` corrected the false fetch claim, including the matching map row. Separate native fixtures verify both target-plus-branch and branch-only routes, and Fable 5.1 reviewed the wording. The benchmark nevertheless ends at 51/52 strict and 52/52 state passes, and does not improve efficiency. Fable recovery r2 skipped `but skill`, grepped the discovery stub for relevant instructions, and attempted blocked `git restore --staged notes/private.md`. All eight task-state checks pass. This failure executes no fetch/update commands and never receives the changed content; it does not demonstrate that the fetch correction caused harm. The experimental commit was removed; its useful factual correction remains in private `r04.patch` for separate review. No failed trial was retried.

**Run 5 — reject: no composite improvement.** Commit `242667b016b74cd4c421945628a52f804d3e26de` replaced the opening loop paragraph to allow requested pull/setup to run directly, adding 15 words. All 52 trials pass, but the composite is 1.0043 versus retained run 3, missing the improvement threshold. Fable cold cases start setup directly but still add status before diff. Dirty-update cases still inspect before pull; some broader reads disappear. Astra still updates its own remote branch before pulling and repeats the update. The commit was removed and its patch preserved.

**Run 6 — reject selective-operation preservation.** Starting from run 3, add one sentence stating that successful commit, amend or discard with explicit uncommitted file/hunk IDs leaves unselected changes pending and unchanged. Remove the suggestion that learning what remains uncommitted normally warrants another read. Native tests cover separated hunks, whole-file selection and unrelated files; committed/history discard and failed operations remain outside the claim. Fable 5.1 reviewed the wording.

Both Astra selective trials stopped after committing and cleanup used just diff then discard. Those local savings did not survive the whole-matrix comparison: the composite was 1.01173 versus run 3, with all 52 passes. The commit `78f3fb1925d738e6ac6be6031540a9c4aaa268b9` was removed and its patch preserved. This is an observed local behavior change consistent with the intended instruction; an unchanged comparison has not confirmed the effect. It is not a measured overall improvement.

**Run 7 — reject explicit checkout syntax.** Starting from run 3, the existing conditional workspace paragraph now spells out `but teardown --checkout-to <branch>` and ties that branch to the requested plain checkout. Astra looked up teardown syntax in all six integration trials from runs 1–3; Fable looked it up four times and twice failed with bare teardown. The change keeps the condition that the user requested leaving the workspace. Fable 5.1 reviewed the copy.

Both Astra integration trials still read teardown help; Fable does so once. All four use the target flag successfully. Fable selective-copy r2 fails only `unfinished_only`: it checks status before running the requested Python tests, then leaves their bytecode cache. Source/history/exact-tree checks all pass. That task performs no teardown. The composite is 1.01478 versus run 3, so this is neither an efficiency gain nor a correctness pass. Commit `a15ac9cf931188b72ce16b49bd3fbed1710bcfac` was removed; the patch and failed trial remain preserved.

**Run 8 — reject minimal fetch correction.** Starting from run 3, replace only the false final sentence and matching map row. Pull fetches; branch update does not. Pull first when both target and branch need updating; use pull --check to fetch without integrating for a branch-only update. This differs from run 4's whole-paragraph replacement. Native fixtures verify the behavior and Fable reviewed the copy. Predeclared factual-correction exception: retain only with all 52 strict/state passes and none of the four ratios more than 5% worse than run 3; label correctness-only if the standard 3% efficiency gate is not met.

All 52 trials pass and all four sync trials fetch before one branch update. One Fable trial adds a preview before the real pull. Astra time is 6.19% higher and Fable commands 7.23% higher than run 3, exceeding the declared 5% guard; composite 1.03183. Commit `90083d6911bf613b5136a23bb002bd7a12541eb1` was removed. The factual correction remains useful and is preserved in `r08.patch`. **The retained guide still has the known false fetch claim; performance selection does not make that content publication-ready.**

**Run 9 — reject scoped-uncommit syntax and preservation.** Starting from run 3, add one sentence spelling out committed file/hunk syntax, selected changes returning to pending work, preservation of the source author/message/unselected changes, and the sole-file empty-commit edge. Native receipts verify independent changes and an unrelated descendant, plus the all-content edge. The sentence makes no universal promise about references, SHAs or dependent descendants. The older full skill stated this syntax explicitly; r06 Astra repeatedly consulted help and audited metadata, while Fable consulted the reference or audited it. Fable 5.1 reviewed the copy. Standard strict/efficiency gate applies.

Astra's two recovery trials load the guide and avoid help/repeated author audits; one still checks protected refs. Fable r1 skips the guide and performs 17 non-guide VC calls; r2 loads it, avoids help, but retains result/author checks. The matrix fails when Astra cold r1 stalls after completed setup help and times out at 180.032s, 165.464s after its last native command. No mutation begins and stderr identifies no provider error. The next cold trial passes in 26.111s. Cause is unresolved; this is not evidence the sentence caused the stall. The timeout stays in the mean and original failed results, with incomplete token/tool-event coverage left missing. Independently of that timeout, Fable’s VC-call mean is 6.38% above run 3, exceeding the 5% guard. Commit `a9b21be52fd82ac5a75e097ab6e2af24d2de36ca` is removed. A reviewed failure-specific wrapper archived the exact hashed failed validation and fixtures without creating a passing validation or changing grades; normal input/coverage/credential/archive integrity checks remain enforced.

**Run 10 — reject target-update recipe opener.** Starting from run 3, replace the actual update recipe's first sentence with a task-triggered instruction to run pull first. Other update guidance and explicitly requested preservation work remain applicable. This differs from run 5's generic opening change. Fable 5.1 reviewed the copy. Standard strict/efficiency gate applies.

All 52 trials pass. Fable has its best passing point estimate so far: time is 13.62% lower and VC calls 3.40% lower than run 3. Astra time is 10.96% higher, so the composite gain of 1.79% misses both the 3% threshold and 5% regression guard. All four dirty-update trials still inspect before pull. This is a provider tradeoff, not evidence of a uniform effect. Commit `01155895507d270863b8ed6a1ae1b314a8f981e8` was removed and its patch preserved. Both cold trials pass; the previous timeout does not recur.

**Run 11 — reject diff-ID comment.** Starting from run 3, change only the narrow-read table's first comment to say commit, amend and discard take the uncommitted file/hunk IDs. The existing status-fv target-discovery guidance remains. Cleanup often starts with status then diff; this tests whether naming discard in the first-read comment is enough to avoid that first status. Native scoped-ID checks support the syntax and Fable 5.1 reviewed the copy. Standard gate applies; run 12 is the unchanged confirmation.

All 52 trials pass, but composite 1.11302 misses the gate. Astra's two cleanup trials start with diff then add help/ref/status inspections; Fable skips the guide in r1 and still starts status→diff in r2 after loading it. Commit `d74a42f4b78ff6a08c69fa1d42c50bce6f55bff7` was removed. No claim that this tiny edit caused all the wider variation.

**Run 12 — completed unchanged confirmation of run 3.** No new edit or commit. Restore the retained guide and reuse the exact original frozen run-3 binary, with the same stub/reference, model binaries, effort and k. The source is also rebuilt for the user, with that build hash recorded separately. This was the last allowed matrix. All 52 strict/state checks pass. The selected point estimate and its confirmation are reported separately; they are not an unbiased k=4 experiment. Compared with the original run-3 estimate, Astra time is 9.55% higher and VC calls 11.37% higher; Fable time is 7.26% lower and calls 0.85% higher. Baseline parity is not confirmed.

**Limit exposed by run 4:** served-content improvements cannot guide a trial that never loads that content. Both corresponding Fable recovery trials in run 3 loaded the complete guide and explained the correct pending semantics. Loader reliability is a distinct concern outside this fixed-stub experiment.

## Decision policy and limits

Correctness comes first. An efficiency candidate is retained if the geometric mean of four ratios—each provider's mean time and mean non-guide command count—improves by at least 3% versus the retained parent, with no individual metric more than 5% worse. Correctness fixes can take priority with a documented tradeoff. The target is 52/52 strict and state passes and both provider means at or below the September 8 baseline on time and commands. A target-reaching candidate needs an unchanged confirmation within the budget. If parity is not reached earlier, run 12 is reserved for an unchanged repeat of the final retained candidate, rather than another unconfirmed selection.

Historical comparisons are descriptive: product versions, instruction content and repetition counts differ. Adaptive k=2 experiments are exploratory and vulnerable to noise and selection bias. Counts do not identify unnecessary work on their own; traces and user requirements are needed. Provider token categories remain separate, and missing cost or reported zero reasoning is not evidence of zero actual cost or reasoning.

## Scenario comparison: baseline and final confirmation

Means per trial. Baseline uses k=5; confirmation uses k=2. VC calls exclude guide loading and identified automatic probes. Each confirmation scenario passes 2/2 strict and state checks. Baseline state checks all pass; its four Fable protocol failures occur in recovery and sync. This is a descriptive comparison across historical product/reference/instruction versions.

### Fable 5.1

| Scenario | Baseline seconds | Confirmation seconds | Baseline VC calls | Confirmation VC calls | Baseline strict |
|---|---:|---:|---:|---:|---:|
| Selective validation | 17.52 | 14.25 | 2.40 | 3.00 | 5/5 |
| Multi-amend | 16.39 | 16.32 | 5.00 | 5.50 | 5/5 |
| Split commit | 19.57 | 31.20 | 7.00 | 8.50 | 5/5 |
| Reorder commits | 12.40 | 12.94 | 2.00 | 2.00 | 5/5 |
| Squash commits | 15.27 | 13.67 | 3.00 | 3.00 | 5/5 |
| Dirty branch update | 31.98 | 33.94 | 15.00 | 21.00 | 5/5 |
| Scoped review | 22.47 | 20.09 | 3.20 | 6.50 | 5/5 |
| Cold checkpoint | 22.62 | 31.30 | 7.80 | 8.00 | 5/5 |
| Cleanup noise | 16.22 | 24.97 | 3.00 | 5.50 | 5/5 |
| Recover amend | 24.02 | 19.83 | 6.60 | 6.00 | 3/5 |
| Dual-source sync | 42.52 | 33.93 | 19.60 | 19.50 | 3/5 |
| Integrate topics | 44.75 | 32.39 | 21.20 | 17.50 | 5/5 |
| Selective copy | 29.08 | 30.38 | 10.00 | 12.50 | 5/5 |

### Astra

| Scenario | Baseline seconds | Confirmation seconds | Baseline VC calls | Confirmation VC calls | Baseline strict |
|---|---:|---:|---:|---:|---:|
| Selective validation | 23.46 | 21.49 | 2.00 | 2.50 | 5/5 |
| Multi-amend | 25.84 | 24.74 | 5.00 | 5.50 | 5/5 |
| Split commit | 28.03 | 27.55 | 7.00 | 7.00 | 5/5 |
| Reorder commits | 23.20 | 21.44 | 2.00 | 2.00 | 5/5 |
| Squash commits | 24.49 | 22.34 | 3.00 | 3.00 | 5/5 |
| Dirty branch update | 39.14 | 41.37 | 6.20 | 15.50 | 5/5 |
| Scoped review | 23.15 | 26.31 | 2.20 | 3.00 | 5/5 |
| Cold checkpoint | 32.39 | 35.80 | 8.20 | 10.50 | 5/5 |
| Cleanup noise | 24.09 | 28.68 | 2.00 | 7.50 | 5/5 |
| Recover amend | 26.43 | 24.02 | 2.20 | 5.50 | 5/5 |
| Dual-source sync | 50.93 | 51.38 | 15.40 | 19.50 | 5/5 |
| Integrate topics | 56.15 | 57.36 | 15.60 | 23.50 | 5/5 |
| Selective copy | 61.27 | 34.25 | 18.40 | 12.50 | 5/5 |

### Other effort measures

These means include guide overhead where stated. Model tool calls are a different unit from native VC calls.

| Provider/version | VC calls including guide | Model tool calls |
|---|---:|---:|
| Astra, baseline | 6.86 | 6.85 |
| Astra, selected run 3 | 9.12 | 7.50 |
| Astra, confirmation | 10.04 | 7.31 |
| Fable, baseline | 8.14 | 4.62 |
| Fable, selected run 3 | 10.08 | 5.50 |
| Fable, confirmation | 10.23 | 5.27 |

Provider-native token and reported cost categories, with their reporting denominators, are preserved in [the confirmation token report](/Users/kiril/.local/state/butgym/served-guide-loop-20260911/r12/token-report.json). Missing categories remain unreported; reported reasoning counts do not represent all internal reasoning. The [full comparison](/Users/kiril/.local/state/butgym/served-guide-loop-20260911/r12/comparison.md) includes the earlier stub runs and selected result.

## Timing variation with matching counts

Fable's two run-5 reorder trials took 34.870s and 13.972s with the same counts: two non-guide native VC calls, one guide call and four model tool calls. Across runs 1–10, all 20 Astra squash trials had three non-guide calls, one guide call and four model tool calls, yet ranged from 16.586s to 26.687s. Matching counts do not prove identical outputs, token volume or time spent inside each step.

Run 10's Astra slowdown spans 10/13 scenarios despite nearly flat total non-guide calls. Fable improves in 9/13, but dirty update, cleanup and sync contribute 95.5% of its net reduction across scenario means. These observations show why an adaptive k=2 timing win needs an unchanged repeat; they neither identify provider latency as the cause nor isolate a guide effect. The per-run comparison JSON `rows` and `scenarios` preserve the underlying values.

## Evidence

- [Detailed working record](/Users/kiril/.local/state/butgym/served-guide-loop-20260911/LEARNINGS.md) and [final status receipt](/Users/kiril/.local/state/butgym/served-guide-loop-20260911/FINAL-STATUS.json).
- VM ledger, patches, copy reviews and helpers: `/home/hermes/.local/state/butgym/served-guide-loop-20260911/`.
- Frozen run inputs, per-trial results, transcripts, comparisons and verified fixture archives: `/home/hermes/.local/state/butgym/served-guide-20260911-rNN/`.
- Native pending/fetch evidence: `/home/hermes/.local/state/butgym/served-guide-native-pending-aevcug69/`.
- Original branch commit: `89d4e804e2810aaba021612fff6d449a0ea4214e`.

Bulky and private evidence stays outside the repository. This reading document remains uncommitted.
