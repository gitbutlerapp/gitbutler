# What actual usage says about the GitButler main skill

**Preserve the short task recipes; improve the decisions around them.** The strongest evidence concerns choosing a sufficient inspection, using complete selectors, understanding which state But represents, recovering from a specific error, and recognizing when the requested outcome is complete. A longer command catalog is not the obvious answer to the remaining performance gap.

This research combines an exhaustive scan of retained structured records with manually reviewed command/output sequences. It supplies empirical input to a separate agent rewriting the main skill. It neither rewrites that skill nor evaluates prompting literature. The [brief](skill-rewrite-brief.md) is the compact handoff; the [coverage matrix](skill/coverage-matrix.md) connects findings to the existing content. Observations, supported interpretations and untested hypotheses are distinguished throughout.

## 1. What was examined

### Benchmark corpus

The inventory reconciles **1,254 distinct raw result records**, with retained native command traces and normalized tool events. It includes the September 8 baseline across But/Git/JJ, September 9 and 10 stub controls, six completed minimal-guidance iterations, the stopped seventh iteration, and prebaseline/canary/model checks. The exhaustive extraction contains 17,475 corrected native task-command records and 7,329 observed tool-call records across those strata and arms. Those two units must not be added together. [Inventory and schema](/Users/kiril/tmp/but-skill-evidence-2026-09-10/benchmark/README.md); [source manifest](/Users/kiril/tmp/but-skill-evidence-2026-09-10/benchmark/corpus-manifest.json).

The main behavioral comparison is **754 complete But trials: 377 per provider**, comprising 65 baseline trials and eight subsequent 39-trial cohorts per provider. Every complete cohort covers the same 13 scenarios; baseline k=5, subsequent k=3. The canonical models are `gpt-6-astra` and `claude-fable-5-1`, both configured at **medium effort**. Fixture “cold-adoption” describes repository/skill adoption, not an API cache condition. A further 58 But results from stopped i07 are marked partial and excluded from complete-suite comparisons. [Trial data](/Users/kiril/tmp/but-skill-evidence-2026-09-10/benchmark/trials.csv).

Another 260 canonical baseline records cover Git/JJ. The 182 auxiliary records are useful for infrastructure and historical context, but not additional canonical replications: 21 canaries, 140 prebaseline records, 12 model-validation trials and nine abandoned early-baseline records. Thus 754 complete But + 58 partial But + 260 baseline Git/JJ + 182 auxiliary records reconcile to 1,254. Twenty-seven retained historical report files are a separate report-only source; their June/July raw transcripts were not found in the VM checkout. For example, a July report records 359/360 canonical passes, including 120/120 But, but does not permit an independent command-level behavioral audit. [Historical report text and provenance](/Users/kiril/tmp/but-skill-evidence-2026-09-10/benchmark/historical-reports.json).

The Fable baseline included automatic platform Git probes. Reapplying the frozen predicate independently removes exactly **575 probes** from the But arm, changing its raw 1,104 native count to 529 task-native commands. All comparison tables use that correction; original grades and files remain unchanged. This matters enough that uncorrected old reports should not be used to compare skill efficiency. [Reconciliation](benchmark/benchmark-findings.md).

### Donated records

All **2,658 files** under the supplied trace corpus were inventoried: 1,677 Codex, 466 Claude, 514 Pi and one OpenCode-labelled export. The last file is a pretty JSON array of 226 file diffs, not an execution transcript. Its source code and examples are excluded from command counts. The other 2,657 files were scanned as agent records. Files and sessions are not people or independent tasks. [Codex findings](donated-codex/donated-codex-findings.md); [Claude/Pi findings](donated-other/donated-other-findings.md).

| Source harness | Files scanned | But-containing submitted units | Meaning |
|---|---:|---:|---|
| Codex | 1,677 | 12,703 outer tool submissions | 12,788 literal shell payloads within them; 2,164 are static nested JS candidates |
| Claude | 466 | 4,146 shell tool payloads | Deduplicated actual submitted payloads; compound fragments may be conditional |
| Pi | 514 | 1,761 shell tool payloads | Three lack a paired result; many use a local development binary |
| OpenCode-labelled export | 1 | 0 | File diffs, not tool executions |

The shell parsers intentionally avoid counting commands quoted in skill text, code, user messages or heredocs. Submitted fragments are not proof of native execution. Stable call IDs and identical inputs are used to identify inherited duplicate records; a repeated legitimate command is retained. Aggregate tool exit status is not indiscriminately assigned to every inner fragment. [Methods](METHODS.md).

The donations are not a clean “production usage” comparison group. Codex primary metadata gives 637 files with benchmark path hints, 704 with GitButler development hints, and 336 other/unknown. Claude's overlapping context tags identify 1,336 But payloads in benchmark-path source files and 2,270 in product-development working directories, with 101 overlapping. Pi is especially development-heavy: 1,711/1,761 But payloads use a product-development cwd. These are context hints, not proof every command was a benchmark or deliberate negative test. Ordinary website and publishing-tool cases are separately identified. [Corpus composition](donated-other/donated-other-findings.md); [Codex context analysis](donated-codex/donated-codex-findings.md).

Model metadata is preserved rather than inferred from the client. Pi contains OpenAI models; the recognized Claude donation calls use older Fable/Opus/Sonnet identifiers. The donated records do not provide a controlled Astra versus Fable 5.1 replication. Agent CLI versions, But versions and transcript-format versions are different fields.

### Present instruction surface

The captured main body is **3,591 words, 22,806 bytes**. After removing frontmatter it is byte-identical to the September 10 served core. The delivery implementation embeds this same main file for full installation and CLI core serving, so the requested rewrite affects both modes. Reference delivery differs: full installation bundles static reference content, while the CLI renders a generated reference. A reference update is therefore a separate potential influence on task paths. [Content audit and source hashes](skill/content-audit.md).

## 2. What performs well

The benchmark gives unusually stable positive evidence for three compact workflows. Across all nine complete studies, Astra's selected-commit, multi-amend and split scenarios use approximately **2, 5 and 7 non-guide native calls**, respectively. Their final states pass throughout. Both providers also have short successful cases for recovery, squash, reorder and cleanup. [Every-scenario scorecards](benchmark/comparable-scorecards.md).

The selected commit succeeds with `but diff`, followed by one commit selecting the returned IDs and creating the destination branch. Multi-amend uses a detailed status and diff to obtain targets/sources, then three amendments. Split uses the prescribed reconstruction order and moves the preserved top commit once. These are more than memorized syntax: they encode an economical first read, object identities, target/source relationships and ordering constraints. Removing them without preserving those facts would throw away observed strengths. [Success sequences](benchmark/benchmark-findings.md).

The donated evidence corroborates a narrow but important piece of this result. In ordinary website work, a diff supplies the relevant file/hunk identity and `but commit -b … <file-id>` creates the branch and commit immediately. There is no separate branch-creation preflight in that local sequence. In other cases a successful amendment prevents a conditional help fallback from running, and a successful PR-creation command already performs the push. These examples support making result guarantees and implicit work legible. They do not prove that every surrounding task should end at the first successful command. [Claude cases C05, C10 and C12](donated-other/evidence-cases.md).

The strength is not universal across all tasks or tools. In the same-day baseline, Astra's But arm is faster than Git for multi-amend and split, but slower for final integration and selective copy. Fable's But arm is also slower than Git in several integration/setup scenarios. The full three-arm table is retained; the skill rewrite should address the workflow facts behind each scenario rather than optimize a single pooled leaderboard. [Same-day But/Git/JJ comparison](benchmark/comparable-scorecards.md#sept8-three-arm-context).

## 3. Where the current performance gap comes from

The following summary uses mean agent seconds and mean **native VC calls excluding guide**. It includes all trials in each balanced cohort, not only successes. Zero guide calls in the full baseline does not mean the injected skill had no context cost.

| Provider and study | Strict / state passes | Trials | Seconds | Non-guide native calls | Model tool calls |
|---|---:|---:|---:|---:|---:|
| Astra, Sept8 full | 65 / 65 | 65 | 33.74 | 6.86 | 6.85 |
| Astra, Sept9 stub | 39 / 39 | 39 | 44.29 | 15.62 | 7.77 |
| Astra, Sept10 control | 38 / 39 | 39 | 37.73 | 16.36 | 7.59 |
| Astra, i06 | 39 / 39 | 39 | 35.82 | 11.44 | 6.90 |
| Fable, Sept8 full | 61 / 65 | 65 | 24.22 | 8.14 | 4.62 |
| Fable, Sept9 stub | 37 / 38 | 39 | 28.76 | 10.05 | 5.08 |
| Fable, Sept10 control | 37 / 39 | 39 | 23.70 | 9.23 | 5.03 |
| Fable, i06 | 39 / 39 | 39 | 24.92 | 8.13 | 5.18 |

[Reproducible whole-study rows](/Users/kiril/tmp/but-skill-evidence-2026-09-10/data/benchmark-study-summary.csv); [all intermediate cohorts and scenarios](benchmark/comparable-scorecards.md).

The complete i06 cohort achieved 78/78 strict and state passes. Fable's aggregate non-guide count reached the baseline level; Astra remained about **6.2% slower with 66.7% more non-guide native calls**, while its model tool-call count was almost unchanged. This is a useful result, not confirmed general parity. Fable's cold-adoption scenario remained slower despite its overall average. The stopped i07 prefix cannot settle the question, and the one-sentence stub change ultimately put into the PR was never tested alone.

The cohorts are sequential and adaptively selected. Product revision, reference, top-level instructions and delivery changed between baseline and later controls. Later candidate comparisons held more of those fixed, but k=3 and sequential execution still do not isolate a sentence's causal contribution. A main-skill rewrite needs its own evaluation rather than inheriting an improvement percentage from this loop. [Methods](METHODS.md).

### Astra: audit depth, more than more tool turns

Post-mutation inspections appear in **193/377 Astra** and **195/377 Fable** complete trials. The near equality is revealing: simple “did it check again?” rates miss the difference in audit depth. Python-attributed Git subprocesses appear in **127 Astra trials and one Fable trial**. One generated script can contain dozens of native Git reads inside one tool call. [Pattern data and interpretation](benchmark/benchmark-findings.md).

The clearest case is reorder. Astra makes the requested anchored block move, then compares old/new commit messages and patches in **all 24 completed stub/candidate repetitions**, versus none of five full-skill baseline repetitions. Its i06 reorder mean is 32.7 non-guide calls and 45.6 seconds; Fable's is two calls and 15.2 seconds. The mutation itself is compact for both. The extra work is establishing preservation afterward. Crucially, the core's reorder recipe does not explicitly forbid these audits: it says to move the block once, and the task asks for preserved content. This is an evidence-sufficiency question, not a proven failure to obey a blanket prohibition. [Paired reorder examples](benchmark/benchmark-findings.md).

Dual-source sync prevents an overbroad explanation. A large audit in one i06 Astra repetition drives an aggregate increase of 21 post-mutation Git reads over i05, while But calls and mutations remain unchanged. That repetition adds 22 reads; changes in the other two offset one. Its four-commit metadata/patch audit has a precedent in one of five full-skill baseline trials. The native audit consumes 902 ms; the complete post-mutation tail takes 30.81 seconds. The record does not separate generated-code production, provider waiting, client roundtrips and final-response time enough to call the remainder “reasoning.” [Exact audit and baseline precedent](benchmark/benchmark-findings.md).

Nor are all 127 Python cases waste. Twenty-four are scoped-review trials where structured comparison of committed JSON directly answers the question. Integration and sync also request exact preservation properties that ordinary status output does not independently prove. The useful distinction is between an additional check that supplies a missing fact and another display of a fact already sufficiently established. The report retains both kinds of examples to prevent a blanket anti-verification recommendation.

Dirty-update traces show a different avoidable tail: exact commit-message byte comparisons discover a terminal newline difference, followed by reword attempts that return “No changes to commit message.” Twelve Astra reword dispatches occur across the complete corpus; dispatch does not mean twelve history changes. The later i05/i06 examples avoid this particular detour. Recovery also sometimes opens an unnecessary index investigation after the requested But uncommit already succeeds. These are narrower, actionable patterns than “Astra thinks too much.” [Failure and recovery sequences](benchmark/benchmark-findings.md).

### Fable: concise paths, with state and policy exceptions

Fable often follows the short mutation recipe and stops, especially in reorder and squash. Across the complete But corpus it has **360/377 strict passes and 369/377 state passes**. Astra has 376/377 strict and 377/377 state passes. These pooled counts summarize the audited records; they are not independent population estimates across a fixed randomly sampled workload. [Trial outcomes](/Users/kiril/tmp/but-skill-evidence-2026-09-10/benchmark/failure-index.json).

The eight complete Fable state failures all concern `unfinished_only` in selective copy. Independent archive inspection confirms Python bytecode residue, while the other eleven scenario checks pass. The harness's broad `PROTECTED_HISTORY_DAMAGE` label would misdescribe the actual result if repeated without examining those checks. Partial i07 contributes one more instance of the same residue mechanism, not a new category of history corruption. [Independent residue verification](/Users/kiril/tmp/but-skill-evidence-2026-09-10/benchmark/copy-residue-verification.json).

The additional nine Fable strict failures have correct final state. Five involve a blocked raw Git fetch; three recovery trials involve raw index-reset/restore attempts. One is a malformed `git -C workspace-origin.git` with no subcommand, rejected by the conservative guard. Astra's sole strict failure is a read-only `git reflog main -5` rejected by that guard. Preserve the canonical grade while distinguishing genuine forbidden mutations, malformed invocation and read-only classification limitations. [All strict failure capsules](benchmark/selected-evidence-index.md).

## 4. A new finding: verification strategy explains the residue pattern

Across **29 completed selective-copy trials per provider**, Astra always submits test/verification code using Python's no-bytecode mode. Fable uses three successful alternatives and one failing pattern:

| Strategy visible in submitted calls | Astra trials | Fable trials | State outcome |
|---|---:|---:|---|
| No-bytecode tests/verification | 29 | 1 | All pass |
| Explicit workspace-cache cleanup, without no-bytecode mode | 0 | 18 | All pass |
| Tests only in an extracted temporary tree, then remove that tree | 0 | 2 | Both pass |
| Neither suppression nor workspace-cache cleanup | 0 | 8 | All fail `unfinished_only` |

The six Astra cases without the literal fixture test command were inspected: four execute the test code in memory, two run it in a `TemporaryDirectory`; all use `-B`. Two first try an unavailable interpreter spelling and then retry with `python3`. The two Fable temporary-only cases never run the tests in the main workspace. This rules out a simplistic “no cleanup command means failure” interpretation. [Per-trial findings, event IDs and reproduction script](data/copy-side-effects.md).

Two failure sequences matter for instruction design. In one, status is checked before tests, so the later cache side effect is unseen at that point. In another, the cache is explicitly visible, but the agent calls it harmless and leaves deletion to the user. Partial i07 even cleans its temporary extraction while leaving the workspace cache. The issue is neither always missing inspection nor always missing cleanup vocabulary. It is responsibility for reaching the requested final workspace after all supplementary work.

This supports a general completion property. It does not justify adding `python -B` to a version-control skill, mandating tests for simple history operations, or deleting every unfamiliar file. Successful agents prevent side effects, confine them to owned temporary directories, or remove specifically created artifacts. User files, requested deliverables and pre-existing work are different objects.

## 5. What the donated sequences add

### Complete IDs and clear argument roles

Manually paired ordinary-use sequences show an agent passing only hunk suffixes and receiving an ID error, then succeeding with full `file:hunk` IDs. Another loses required printed disambiguation; another must refresh an ID after a preceding mutation. An amendment submitted without its explicit target fails and succeeds when the target flag is added. These cases support precise selector shape, source/target roles and operation-specific freshness—not a broad requirement to rerun status before every command. [Claude/Pi cases C01–C03, C06–C07](donated-other/evidence-cases.md).

This complements the benchmark: two Fable baseline commit attempts also use bare hunk suffixes, whereas repeated successful current fast paths use complete IDs. The connection is stronger than a high frequency of the word “ID” in transcripts because the casebooks show actual input, native error, changed input and successful output.

### Git habits and historical version drift

Actual donated tool results reject `but status --short`, unsupported diff cardinality/pathspec forms, `diff --stat`, and PR flags borrowed from `gh`. These are useful boundary examples. But apparent syntax errors also track a real historical migration: one installed CLI rejects today's `commit -b`, while a later session must replace the old positional branch/`--changes` form with the new syntax. A comma-related error in an old transcript can mean individually stale IDs, not unsupported comma splitting. [Codex cases](donated-codex/evidence-cases.md); [Claude/Pi cases C04, C05, C08 and C10](donated-other/evidence-cases.md).

The transferable lesson is to expose current task recipes and respond to actual local-version errors. It would be wrong to teach every historical spelling, infer a current product bug from an old rejection, or treat the file's agent version as the version of But.

One Codex diff-cardinality repair also exposes a second problem: the separate, syntactically valid per-ID diff returns 68,978 original tokens before truncation. Across But-containing batches, 512 initial results have truncation markers. These do not measure wasted time, but they show that choosing a valid selector does not guarantee a usable amount of evidence. Task-appropriate output scope matters alongside command count. [Codex C02 and inventory](donated-codex/donated-codex-findings.md).

### Environment, dependency and publication boundaries

Pi's development-heavy corpus contains missing-PATH errors followed by successful use of a known build-output binary. Other cases encounter setup-required state, dependency refusal and explicit command deprecation. One repeated wrong-context status returns the identical setup-required error twice. These illustrate narrow recovery questions: correct executable, repository context, state prerequisite or command syntax. They do not support treating every nonzero result as agent misunderstanding. [Cases C09, C11 and C13](donated-other/evidence-cases.md).

PR creation and branch dependencies are visible in donated work even though the thirteen-scenario matrix provides limited coverage of them. A failed dependency operation can be atomically refused; the next useful action depends on ownership and stack placement, not blindly retrying. Publication commands have implicit effects and noninteractive input requirements. These topics should not disappear simply because they occur less often than status or diff. [Casebook](donated-other/evidence-cases.md); [section matrix](skill/coverage-matrix.md).

The permission-question scan does not substantiate a “permission ceremony” problem. Reviewed questions include changing already-pushed history, publishing local fixes, missing target names and main/master ambiguity. Other keyword candidates are product-design questions or ordinary English “but.” Likewise, repeated But inspections may follow intervening edits or changed output. The original requests and intervening state matter. [Question and sequence limitations](donated-other/donated-other-findings.md).

### Why raw keyword mining would have misled us

Broad error terms appear in source diffs, test summaries containing `0 failed`, and output from another process in a batch. A seeded review of ten cue-only nonerror payloads per harness finds six Claude and nine Pi examples with no But execution failure. Some remaining aggregate-success cases contain real failures masked by a later successful command. Neither keyword nor outer exit code is a complete oracle. [Validation sample](/Users/kiril/tmp/but-skill-evidence-2026-09-10/donated-other/keyword-validation.json).

The Codex scan separately identifies 223 nonzero single-CLI-only batches with anchored syntax markers; their primary file contexts include 129 development, 45 benchmark and 49 other/unknown. Even this narrower candidate set is not a real-world model-mistake rate: deliberate negative probes and historical versions remain possible. The casebooks provide the reviewed error/recovery evidence. [Codex extraction and qualification](donated-codex/donated-codex-findings.md).

## 6. Skill delivery explains some friction, not all of it

In the completed benchmark corpus Astra makes 297 bare guide, 85 reference and 15 full-guide native calls; Fable makes 270 bare, 89 reference, 12 full and one concepts call. No standalone examples call is observed. Full output can contain examples, so this is not evidence the examples were never exposed or have no value. [Guide counts](benchmark/benchmark-findings.md).

The early September 9 Fable pattern is concrete: all twelve full-guide calls are piped, ten through `head -400`. Client previews then sometimes show only a small prefix and save a file. If `head` already removed later material, searching the saved file cannot recover it. Bundling documentation with task inspection can also obscure the latter and prompt repeated reads. Subsequent core delivery removes that observed full-output truncation mechanism, yet Astra's audits remain. More text in the main file is therefore not an evidenced fix for the residual audit gap. [Delivery evidence](benchmark/benchmark-findings.md).

Guide presence has three layers: emitted content, client-delivered content, and behavior consistent with the instructions. The exact-current-core flag in normalized tool results does not cover every delivery mechanism. Fable Skill expansion may arrive in an injected user message; the extracted rows retain separate prior-observer receipts for that case. False in an older baseline can also reflect a different version. Neither presence nor absence in one normalized field proves attention or adoption. [Schema details](/Users/kiril/tmp/but-skill-evidence-2026-09-10/benchmark/README.md).

The donated records predate this specific content-loading experiment. Recognized `but skill` calls in Claude/Pi are installation/check commands; no recognized bare/full/reference/examples content loads occur there. Older file reads include development inspection of the skill itself. They cannot serve as a fresh test of the stub loader.

Earlier token analysis also weakens a simple “slower means cache broke” explanation. September 8–9 Astra input increases 26.1% and output 67.7%, but cached-input share stays around 87%; Fable output rises 25.7% while its provider-reported cost estimate declines about 2%. These are provider-specific accumulated usage fields, not document size, invoices or hidden-reasoning measurements. [Usage definitions and prior source](METHODS.md#time-tokens-and-cost).

## 7. Implications for the main-skill rewrite

The current section inventory separates syntax from semantics more usefully than a frequency leaderboard. Command Patterns contains 589 words; IDs 419; the dedicated conflict section 536. Start Here, selected-commit recipe, Command Patterns and the Git-to-But map repeat some commit information. There is a real opportunity to evaluate consolidation. But branch creation, command defaults, target direction and mode scope are decision facts that should not vanish with duplicated flag lists. [Exact content allocation](skill/content-audit.md).

One source-level inconsistency deserves deliberate resolution: the main-update recipe routes directly into edit-mode conflict handling, while the dedicated conflict section prefers an apply loop and reserves edit mode for running code against a resolution. Both are supported paths; this is a competing-default question, not proof either command is defective or that those lines caused a particular delay.

For the writing agent, the evidence suggests the following priorities:

1. Preserve the first-read and multi-operation recipes that consistently produce short correct paths.
2. Explain the state/identity facts an agent must know to choose those paths: complete selectors, explicit targets, fresh IDs when needed, But pending state, and ordinary-checkout transitions.
3. Make output sufficiency conditional on the actual task requirements. Distinguish the next selector needed for a mutation from a final ritual display, and a genuinely missing preservation fact from a duplicated display.
4. Clarify completion after supplementary tests or scripts, including responsibility for their effects without expanding cleanup scope.
5. Keep error recovery tied to the concrete refusal or missing prerequisite. Treat historical mismatch as diagnosis, not a reason to preload every old command.
6. Evaluate consolidation of repeated grammar and overlapping reference content. Preserve semantic defaults and rare consequential recovery paths until coverage supports moving or removing them.

These are empirical design questions, not proposed copy or proof of an optimal structure. [Section-by-section evidence and gaps](skill/coverage-matrix.md).

## 8. How to use and challenge this pack

The [54 benchmark capsules](benchmark/selected-evidence-index.md) include all nineteen complete/partial strict failures, twenty-six low-command successful i06 cases covering every scenario/provider, and nine heavy-audit precedents. The successful cases are minima within three repetitions, not global optima; selected failures/audits are illustrative, not a prevalence sample. Each capsule retains task requirements, output excerpts, final checks and original source/hash pointers.

The donation casebooks supply exact local file/line pointers, model metadata, submitted commands and paired results. Machine-readable extracts retain candidate classifications so the other agent can audit a claim, find counterexamples or inspect a different slice without launching another benchmark. Broad raw transcripts are not copied wholesale into the synthesis.

No experiment here establishes that a particular sentence is necessary or sufficient. No hidden chain of thought is mined, and elapsed gaps are not labelled reasoning time. Rare worktree, collaboration, dependency, authentication and publication conditions remain under-tested. Their absence is an evidence gap, not evidence of irrelevance.

For a future rewrite comparison, retain separate checks for requested state, command policy, artifacts, native work and model roundtrips; use the same product/reference/top-level instructions across both delivery modes; examine provider/scenario results rather than only an average. Add a small set of targeted uncovered workflows before deleting their semantics. The existing loop remains stopped, and no new trial is implied by this recommendation.
