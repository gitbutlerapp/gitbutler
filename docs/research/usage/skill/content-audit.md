# Main skill content inventory

This is an inventory of the instruction surface, not a replacement skill or an attribution of performance to individual sentences. All line references below refer to the frozen local [main skill snapshot](/Users/kiril/tmp/but-skill-evidence-2026-09-10/skill/current-main-skill.md). Its source and SHA256 are recorded in [source-manifest.json](/Users/kiril/tmp/but-skill-evidence-2026-09-10/skill/source-manifest.json).

The snapshot has 220 lines including frontmatter, 3,591 whitespace-delimited words in its body, and 22,806 body bytes. Removing the seven frontmatter/separator lines produces exactly the September 10 served core, SHA256 `97dfa58d80da353ddc1342d186cbb57a98cfff5cdb4970dbe4292577524606e2`. The comparison is saved in [main-vs-frozen.diff](/Users/kiril/tmp/but-skill-evidence-2026-09-10/skill/main-vs-frozen.diff), which is empty. This makes the September 10 experiment findings directly relevant to the present main skill. It does not establish identical instructions in older donated sessions or the September 8 baseline.

## Delivery surfaces

The source registers the main skill with no alternate renderer; the core printed by the CLI comes from the same embedded `SKILL.md` that is installed in full mode. Printing removes trigger frontmatter. Stub installation retains that frontmatter and substitutes the discovery-stub body. Thus a change to this main body reaches both full installation and the core served to stub users. [1]

The reference is different: CLI serving uses the generated reference renderer, while full installation retains the bundled static reference. This distinction must remain in historical comparisons. A reference lookup, help lookup, or newly available command can change the execution path even when the main skill is unchanged. The static reference snapshot is included as provenance, not as a claim that it is what every stub trial received. [1]

## Content allocation

| Section | Snapshot lines | Words | Content type |
|---|---:|---:|---|
| Start Here | 12–40 | 141 | Choosing an inspection and the selected-change fast path |
| IDs | 41–53 | 419 | Object/address grammar, lifetime, batching constraints |
| Non-Negotiable Rules | 54–61 | 220 | Tool policy, result sufficiency, landed branches, help and narration |
| Command Patterns | 62–87 | 589 | Command syntax plus argument/default behavior |
| Update workspace from main | 90–102 | 273 | Main-update semantics and conflict recovery |
| Commit selected files or hunks | 103–111 | 150 | Selection, commit, mixed-hunk edge case |
| Amend into existing commit | 112–116 | 63 | Targeted inspection and amendment |
| Split an existing commit | 117–133 | 331 | Two-way split and reconstructing a larger split |
| Reorder commits | 134–142 | 144 | Display order, anchors, moving a block |
| Squash commits | 143–150 | 107 | Source/target selection and sequencing |
| Stack existing branches | 151–156 | 62 | Stacking and an invalid reconstruction path |
| Create or manage pull requests | 157–162 | 123 | Publishing a stack, noninteractive input, auth |
| Dependency conflict with another branch | 163–170 | 172 | Atomic refusal and recovery hints |
| Resolve conflicted commits | 171–195 | 536 | Conflict model, apply loop, AI fallback, edit mode |
| Conflicts in uncommitted files | 196–199 | 55 | A separate conflict state and resolution interface |
| Git-to-But Map | 200–213 | 124 | Translation from Git operations |
| Notes | 214–220 | 66 | Git reads, freshness notice and further documents |

The machine-readable inventory is [main-skill-sections.json](/Users/kiril/tmp/but-skill-evidence-2026-09-10/skill/main-skill-sections.json). Counts describe text allocation, not model tokens, attention, usefulness, or an optimal length. Heading detection excludes comments inside code fences.

## Repetition that can be evaluated for consolidation

The snapshot repeats the selected-change fast path in Start Here and its task recipe; the commit grammar also appears in Command Patterns and the Git-to-But Map. `--status-after` occurs 11 times; `but status` occurs 22 times; `but diff` 15 times; and `but commit` 16 times. These are literal string counts, including examples, not independent instructions or usage counts. [2]

Several repetitions carry distinct information. The general result-sufficiency rule differs from refreshing identifiers after a history rewrite; a status call needed to select the next target is different from a final ritual check. Consolidating text without preserving those conditions could remove useful distinctions. Whether repetition helps adoption has not been isolated experimentally.

Command Patterns is a plausible place to examine overlap with the generated reference: it contains both grammar and semantic facts. The reference can provide flags and accepted arguments, but facts such as implicit branch creation, source/target direction, which stack receives an untargeted commit, and the scope of a worktree operation are decision-relevant. Evidence of a syntax lookup is not evidence that the corresponding semantic fact can safely disappear from the core.

## Specific semantic distinctions to preserve in the evidence review

**Result sufficiency is conditional.** Lines 39, 57 and 108 discuss avoiding additional status/diff when the mutation result already supplies what is needed. They do not establish that every mutation result proves every requested invariant. The reorder recipe prescribes one anchored block move and prohibits moving the members again; it does not explicitly prohibit preservation audits. Repeated verification must be judged against both the task requirement and the output actually delivered to the agent.

**Main update is not every kind of integration.** Lines 90–102 describe updating applied work from main. The body has no dedicated recipe for the benchmark's two-remote reconciliation or final integration into an ordinary Git checkout. Ending after the main-update step can leave those tasks incomplete. The absence of a recipe can identify an information need, but it does not by itself justify adding a benchmark-specific recipe.

**Git reads and Git writes are separate policies.** Line 56 directs writes through But; line 216 permits read-only Git inspection. A raw Git read is not automatically a protocol violation. It can still be redundant or answer the wrong state question. Conversely a Git command described by an agent as a harmless preflight may perform a forbidden write.

**Pending But changes and an ordinary Git index are not interchangeable questions.** The main body does not explicitly explain this distinction. Existing benchmark findings show why it matters, but a universal claim that the index never matters would be false in scope: some tasks explicitly request an unstaged file or a clean ordinary checkout after teardown. A proposed rewrite needs the operation context, rather than one blanket rule about Git's index.

**A successful command and a completed task are different.** Temporary files introduced by supplementary checks can remain after the requested history change succeeds. Cleanup ownership is a task-completion consideration; it must not authorize removing pre-existing user files or requested deliverables. This topic is not explicit in the current main body. Its experimental evidence belongs in the benchmark report, with the k=3 and sequential-treatment limitations preserved.

## Internal routing and coverage questions

The main-update recipe at line 95 points directly to edit-mode conflict resolution (`but resolve <commit>` and `finish`). The dedicated conflict section at lines 175–194 prefers the apply loop and reserves edit mode for running code against a resolution. Both paths are described, but they give different default routes. This is a source-level consistency question for the rewrite; it is not a claim that either command is broken or that an observed delay was caused by these lines.

The IDs section is detailed about uncommitted, committed, branch, and commit selectors. The conflict-apply section adds a local exception about rewritten commit identities and branch selectors. Treat those as operation-specific lifetimes. Collapsing all identifiers into a single rule risks losing distinctions that a raw count of identifier errors cannot resolve.

The thirteen benchmark scenarios exercise selective commit/review, amendment, splitting, reorder, squash, dirty update, cold setup/checkpoint, noise cleanup, recovery, dual-source sync, integration and selective copy. They do not amount to comprehensive coverage of PR lifecycle/authentication, real collaborators' concurrent changes, landed-branch dependencies, worktree targeting, all conflict modes, or every fallback. Donated sessions can add examples in those areas, but without fixture oracles their final-state correctness is often unverified.

No section can be declared obsolete merely because it receives few calls in these data. Rare high-consequence operations and error recovery have different value from common fast paths. Candidate reductions should be framed as hypotheses to test, with an explicit statement of which fact would otherwise be supplied by command output, generated reference, product behavior, or another instruction source.

## Sources

1. [Skill delivery source snapshot](/Users/kiril/tmp/but-skill-evidence-2026-09-10/skill/skill-delivery.rs), lines 34–39, 78–110, 456–476 and 1148–1188. Original: `/Users/kiril/src/gitbutler/crates/but/src/command/skill/mod.rs`; hash in source manifest.
2. [Main skill statistics](/Users/kiril/tmp/but-skill-evidence-2026-09-10/skill/main-skill-stats.json), [section inventory](/Users/kiril/tmp/but-skill-evidence-2026-09-10/skill/main-skill-sections.json), [main skill snapshot](/Users/kiril/tmp/but-skill-evidence-2026-09-10/skill/current-main-skill.md), and [September 10 served core](/Users/kiril/tmp/but-skill-evidence-2026-09-10/skill/september10-served-core.md). These are direct file measurements and source observations, not behavioral claims.
