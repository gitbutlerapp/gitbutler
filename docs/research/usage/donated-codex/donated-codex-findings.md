# Donated Codex findings — 2026-09-10

The strongest evidence is about **specific syntax mistakes and their recovery**, plus historical syntax drift. The corpus also contains many post-result inspections, but counting them as wasted work would overstate what the traces prove. This is an offline donation study, separate from the September controlled benchmark; no new trials were run.

## Scope and denominators

All **1,677 JSONL files, 2,028,089,811 bytes, and 785,004 lines** were scanned with zero JSON parse errors. Extracted call timestamps span **2026-02-03 through 2026-08-28**. The source files remain untouched. The complete reproducible inventory is in [manifest.json](/Users/kiril/tmp/but-skill-evidence-2026-09-10/donated-codex/manifest.json), [summary.json](/Users/kiril/tmp/but-skill-evidence-2026-09-10/donated-codex/summary.json), and [sessions.jsonl](/Users/kiril/tmp/but-skill-evidence-2026-09-10/donated-codex/sessions.jsonl); parsing and confidence limits are in [methodology.md](/Users/kiril/tmp/but-skill-evidence-2026-09-10/donated-codex/methodology.md).

| Inventory unit | Count |
|---|---:|
| Tool-call occurrences, all tools | 128,494 |
| Stable-call-ID + payload-deduplicated tool calls | 128,452 |
| Files containing any extracted CLI/skill-file read | 1,616 / 1,677 |
| Files containing extracted But | 1,546 / 1,677 |
| Deduplicated outer tool submissions containing But | 12,703 |
| But-containing literal shell payloads | 12,788 |
| Of those: direct shell / static nested JavaScript candidates | 10,624 / 2,164 |
| Parsed But executable segments | 13,533 |
| Of those: direct shell / static nested JavaScript candidates | 11,060 / 2,473 |
| Parsed Git segments | 11,767 |
| Parsed `gh` segments, kept separate | 3,729 |
| Literal GitButler `SKILL.md` file-read segments | 2,715 |
| Parsed `but skill` segments | 0 |

These are **submitted payload/segment counts, not native executions**. A batch may contain conditional or unexecuted segments; static JavaScript expressions can be skipped or repeated. Six exact repeated CLI-containing calls were collapsed, with every provenance location preserved. All call IDs are stable-looking 29-character `call_` IDs; none was associated with multiple payloads. Distinct IDs are not merged merely because commands match.

Sparse native telemetry corroborates **1,091 CLI-containing shell completions, including 424 containing But**; all match a retained call ID. It does not provide whole-corpus native coverage. Every But outer submission has a paired initial response, but **2,428 lack a reported exit code** and **306 show a running-session/cell marker**. Continuation results can have another call ID. There is no defensible whole-corpus success rate from these initial responses.

## Context and model coverage

| Primary file context, from first session metadata | Files | But-containing outer submissions |
|---|---:|---:|
| Benchmark path hint | 637 | 5,965 |
| GitButler development path hint | 704 | 4,883 |
| Other or unknown | 336 | 1,855 |

A context is a **file hint**, not a per-command purpose verdict. Inherited metadata, fixture setup, deliberate invalid-command tests and product work can coexist. These donations do not represent 1,677 independent users or a random sample of ordinary workflows.

But segment model metadata: GPT-5.5 **6,657**; GPT-5.6 Sol **3,413**; GPT-5.4 **2,116**; GPT-5.3 Codex **1,107**; GPT-5 Codex **99**; GPT-5.2 Codex **58**; GPT-5.3 Codex Spark **49**; GPT-5.4 Mini **34**. No But segment lacks model metadata, and **none is attributed to Astra**. Recorded Codex application versions and all encountered session IDs are in the manifest rows; these are not GitButler binary versions. One manually verified actual But version is **0.5.2152** on August 14 (case C08). Most individual calls lack an explicit contemporaneous But version.

## What agents actually submitted

| But family | Parsed segments |
|---|---:|
| Status | 4,791 |
| Diff | 2,299 |
| Commit | 1,358 |
| Show | 965 |
| Help (`help`, `--help`, `-h`) | 799 |
| Amend | 454 |
| Push | 328 |
| Branch new | 267 |
| Link | 247 |
| Move | 244 |
| Resolve | 178 |
| Branch list / show | 169 / 159 |

Status and diff account for **7,090 / 13,533** But segments. This reflects substantial inspection demand; it does not prove over-inspection. Full family and flag counts remain available in `summary.json` and the normalized [commands.jsonl](/Users/kiril/tmp/but-skill-evidence-2026-09-10/donated-codex/commands.jsonl).

The 2,715 skill-file reads establish an existing file-based guide-loading workflow. No parsed `but skill` command was found in this pre-September donation window. This corpus therefore supplies **no observed runtime stub-loading or `--full` behavior**; do not import the controlled benchmark's guide-delivery conclusions into it. Dynamic/indirect commands remain a documented blind spot.

## High-confidence learnings

1. **Concrete Git-to-But syntax differences create recoverable friction.** In reviewed cases, `but status --short`, `but help squash`, and multi-target `but diff` are explicitly rejected. The agents recover with plain status, `but squash --help`, bare diff or one diff per ID. The multi-target mistake appears in both February and late July. See [C01, C02 and C09](evidence-cases.md).
2. **Current guidance must be distinguished from old executable syntax.** There are **1,207 segments with `--changes`** and **55 `rub` segments** in this historical corpus. Those counts are not current-version errors. In C03, February's CLI splits `--changes nz,pw` and rejects the two nonexistent IDs individually: commas were accepted. C04 shows a real transition failure in July, then successful correction from positional branch/`-c`/`--changes` to `-b` plus space-separated IDs. Recommending present syntax is sensible; retroactively grading all old syntax as misuse is not.
3. **ID resolution needs exact output evidence.** C03 establishes IDs not found and a subsequent status reread. It does not establish whether the IDs were invented, stale, or invalid for another reason. C06/C07 provide positive examples: agents use the exact newly returned commit IDs in subsequent `show` calls. No global “ID misuse” rate is claimed without entity-lifetime tracking.
4. **The compact selected-change recipe works in real donated traces.** C05 shows bare diff followed by a targeted `commit -b ... <ids>`, with creation confirmed and no separate branch creation in that window. This supports the recipe's usability, not a two-call claim for the entire development task.
5. **Stopping rules should be tied to what the result confirms.** C06/C07 show `but show <new-id>` after a successful commit. They are concrete post-result inspections, but the trace pattern alone does not prove they were unnecessary. Preserve legitimate contents/ownership/task-required checks. The evidence supports conditional stopping, not banning all verification.
6. **Output volume and attribution matter.** C02's successful per-ID diff can still produce 68,978 tokens before truncation. Across But-containing batches, **512** initial results have a truncation marker. C08 shows a mixed source-read + version batch containing retired-syntax examples: those examples are not executed commands or failures. Do not turn output keyword counts into behavioral diagnoses.

## Narrow error inventory and exploratory patterns

There are **657 But-containing outer batches with some nonzero exit observed**. Of **10,691 single-CLI-only But batches**, 564 have a nonzero exit; **223 additionally have an anchored syntax diagnostic**. That last subset includes 129 development-context, 45 benchmark-context and 49 other/unknown-context files' submissions. It is a conservative historical diagnostic subset, **not an ordinary-user mistake rate or present CLI failure rate**. Tests can intentionally submit bad commands (C10), and missing final exit telemetry undercounts failures.

Within that 223-batch subset and other single-CLI nonzero diagnostics, explicitly rejected flags include `--short` **54**, `--status-after` **26**, `--stat` **21**, `--json` **21**, and `--oneline` **10**. Flag validity depends on command and historical version. Full call/output pointers are in [syntax-error-batches.jsonl](/Users/kiril/tmp/but-skill-evidence-2026-09-10/donated-codex/syntax-error-batches.jsonl). Broad discovery signals are retained in raw normalized rows but must not be reported as actual failures: source code, help and diffs contaminate them.

| Adjacent retained-CLI pattern | Candidate pairs | Interpretation limit |
|---|---:|---|
| Mutation then status/diff/show | 983 | Includes 497 benchmark-context pairs; not automatically redundant |
| Same exact argv again | 743 | Can be polling, changed repository state, or a real retry |
| Help then help | 448 | Different commands can legitimately need different help |
| Diff then commit | 288 | Local recipe shape; not whole-task efficiency |
| Nonzero batch then changed same family | 261 | Candidate correction; original exit may belong to a mixed batch |
| Nonzero batch then help | 45 | Often appropriate recovery |
| Plain status then status `-fv` | 19 | Extra detail may actually have been needed |

These are adjacent **retained CLI segments**, with other work possibly between them. They are explicitly review candidates in [pattern-candidates.jsonl](/Users/kiril/tmp/but-skill-evidence-2026-09-10/donated-codex/pattern-candidates.jsonl), not waste/retry verdicts. A refined scan found **19 visible assistant permission-or-next-step question candidates**; no user authorization history was exported and no unnecessary-permission rate is inferred.

## Confidence and handoff

Ten purposively selected source windows were manually reviewed across ten files, covering early/later errors, ID resolution, syntax migration, efficient commits, post-result inspection, help discovery and a mixed-output negative control. Every observation and limitation is paired with exact file/line/call-ID citations in [evidence-cases.md](evidence-cases.md). This is qualitative stratification, not a random sample.

The full corpus is covered at the event/schema layer; conservative shell/JavaScript parsing intentionally leaves gaps. Notably, 2,074 substitution-containing payloads, 23 shell-function payloads, 20 shell-lex failures, 308 dynamic nested command values, 184 unresolved nested command fields and 4 nonliteral nested arguments were not expanded; exclusions can overlap. There are also 256 Cargo-run-But and 277 indirect interpreter/remote hints. These missing paths prevent an exhaustive native-command census.

The evidence is strongest for precise syntax/feedback improvements and careful treatment of historical versions. It cannot establish causal speedups, a current-model ranking, an Astra behavior claim, a user-level incidence rate, or a general ban on inspection. No repository, frozen benchmark input, raw donation, or optimization control was changed.
