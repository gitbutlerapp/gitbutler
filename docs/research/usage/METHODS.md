# Evidence definitions and limits

## Scope and source handling

This pack concerns GitButler's `but` CLI. Git commands provide workflow context and GitHub's `gh` CLI is a separate tool. The sources are retained butgym benchmark artifacts, donated agent records under `/Users/kiril/tmp/traces`, and a snapshot of the current main skill and its delivery code. The original records remain unchanged. No benchmark was restarted and no skill language was changed during this research.

The benchmark and donation extracts have different observation units. Read their manifests and extraction summaries before combining counts. The reports deliberately preserve those differences rather than produce one total that mixes submissions, executions, scenarios and sessions.

## Benchmark observations

A benchmark trial is one existing result record for a particular task, agent, tool arm and repetition. Completed balanced matrices, the stopped partial matrix, canaries, aborted runs and prebaseline/model checks are separate strata. Historical summary reports without raw result records remain report-only evidence; they are not reconstructed as fictitious trials.

`strict` and `state` are separate outcomes. A forbidden command attempt can fail the protocol while the final state is correct. Conversely a broad failure-class name can describe a much narrower failed invariant: inspect the named verifier checks and retained output before claiming protected-history damage. Frozen canonical outcomes are never silently corrected to make a candidate look better. Known guard false positives are explanatory annotations.

Native command receipts and model tool calls are different units. One Python or shell tool call can run many Git subprocesses. Guide commands, platform probes, setup and agent-directed task work also need separate accounting. The historical Fable baseline correction excludes the exact established platform-probe predicate; it is not permission to discard other inconvenient commands. Per-trial rows retain correction and provenance fields.

Comparisons within a complete study use the same scenario mix, with per-trial means or equal scenario weights. k=5 and k=3 runs have different sample sizes. The partial final run is an uneven prefix and cannot provide a full-suite parity comparison. Nor can a total pooled across repeatedly optimized copies establish how a new skill will generalize.

Between the September 8 baseline and later stub studies, product revision, served reference, skill delivery and top-level operating instructions changed. Later minimal-stub candidates shared frozen product/core/reference/settings, but they were sequential, adaptively selected k=3 runs rather than randomized isolated tests of each sentence. No individual line's necessity or causal effect has been established by that loop.

## Donated observations

A submitted shell payload is an agent tool call requesting execution. An extracted command fragment is a conservatively parsed segment inside that payload. It is not automatically a separately observed native execution. Conditional branches, shell errors, compound commands, scripts and client permission behavior can prevent a submitted fragment from running. A batch exit code applies to the submitted batch; it must not be copied indiscriminately onto every fragment as proof of success or failure.

Quoted commands in documentation, tool output, user messages, code being written, heredocs and examples are not counted as executions merely because they contain `but`. Dynamic wrappers and uncertain shell constructs are reported as coverage gaps or ambiguous candidates. The extraction scripts and per-corpus notes describe what they recognize.

Stable call/event identifiers and matching content can identify duplicated inherited history. Short ordinal identifiers, repeated legitimate commands and repeated sessions must not be conflated. Deduplication establishes a usable observational corpus, not independence between people, tasks or sessions. Session/file counts are not contributor counts.

Some donated records are benchmark work or GitButler development/reproductions. Those contexts are tagged separately from ordinary repository use when the record establishes them; unknown context remains unknown. The OpenCode-named file is a file-diff array, not an execution transcript, and is excluded from command denominators.

An agent client name is not a model name. Model identifiers are reported from available metadata; Pi can use OpenAI models, and older Codex and Claude sessions are not automatically Astra/Fable. CLI versions and old command spellings are retained where observed. A historical rejected spelling is not evidence that the same problem remains in today's binary.

Real sessions do not have benchmark fixture oracles. Exit zero, a final success claim, or a plausible diff is weaker evidence than a verified final state. Human pauses, long tasks, reconnects and incomplete transcripts prevent straightforward comparison of end-to-end donation durations with short benchmark wall times.

## Interpretation

**Observed:** a command was submitted, a particular result was recorded, a verifier check failed, or a specific line of guide text appeared in a recorded result.

**Supported interpretation:** the subsequent action appears responsive to that result, or a later inspection repeats a fact demonstrably supplied earlier, with the relevant task requirement and intervening mutations checked.

**Hypothesis:** different guidance might change the behavior, a model chose an audit because of uncertainty, or moving a fact to another document might retain performance. These require another experiment.

“After the final mutation” is a useful location, not a waste classifier. Some tasks request exact preservation, remote immutability, tests, clean ordinary checkout or an unstaged file. Some checks add proof the mutation output does not contain. The stronger redundancy finding is a second inspection of already supplied sufficient evidence with no intervening change—not merely the presence of another read command.

The three layers of documentation evidence are also separate: the binary emitted content; a client retained/delivered it; the agent acted in accordance with it. Native stdout does not establish that a clipped or bundled client result exposed the whole guide. Visible guide text does not establish attention, understanding or compliance.

Sparse or absent usage does not establish that a skill section is irrelevant. Rare recovery, publication, multi-agent and worktree operations can have high consequences. A case selected for interesting behavior is not a random prevalence sample. The casebooks include efficient successes to counterbalance failure-driven selection, but population rates must come from the explicitly defined scans.

## Time, tokens and cost

Elapsed time outside a visible tool interval is not a measurement of hidden reasoning. It can include generation, provider waiting, client overhead and unobserved activity. A rate-limit notification alone does not prove throttling. More native commands may increase generated script work even when native execution itself is under a second.

Provider token fields have different meanings. Astra's cached input is included in its reported input total and must not be added again; Fable's ordinary input, cache-read and cache-creation fields are separate. Accumulated usage across turns is not unique document size or peak context. Output tokens include generated commands and final text, not a reliable isolated reasoning measure. Provider-reported cost estimates are not necessarily subscription charges or invoices.

The prior September 8–9 usage analysis found Astra input totals rising 26.1% and output 67.7%, while its cached-input share stayed around 87%; Fable output rose 25.7% while its reported cost estimate declined about 2%. These observations oppose a simple equation between slower, larger context and more expensive. They describe those two studies only. Source: [prior token/cost analysis](/Users/kiril/.local/state/butgym/stub-skill-20260909/token-cost-research.md), with underlying receipt pointers in its companion JSON.

## Private access and reproducibility

Reports and extracted evidence are private, intended for the skill-rewrite agent. Raw donations, benchmark fixtures, credentials and unrelated user conversations are not republished. Source file paths and line/event identifiers allow local verification; remote pointers use `hermes@butgym`. Selected benchmark evidence is also copied into this pack so the main findings can be inspected from the Mac.

Corpus-specific extraction scripts, manifests and source hashes accompany the data. Read the generated findings and schema notes before rerunning scripts: remote analysis uses read-only original sources, and output directories are separate from repositories and frozen trials. Reproduction should never overwrite the original evidence or resume the stopped optimization loop.
