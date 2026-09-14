# Donated Claude, Pi, and OpenCode: evidence for a main-skill rewrite

The strongest transferable evidence is about selectors and command recipes: copy complete IDs, separate source IDs with spaces, distinguish an amend target from its sources, do not import Git/gh flags into But, and recover from the actual error rather than starting a broad verification loop. Successful two-command selected commits also occur. This corpus does **not** support a model leaderboard, a general task-success rate, or a causal claim that a longer skill caused wasted work.

The detailed, source-linked evidence is in [evidence-cases.md](evidence-cases.md). It contains 14 manually inspected cases, including ordinary website/publishing-tool work outside the explicitly identified benchmark and GitButler-development contexts. These are independent evidence *sequences*, not independently identified users.

## Whole-corpus inventory and units

| Measure | Claude harness | Pi harness | OpenCode-labelled export |
|---|---:|---:|---:|
| Files scanned | 466 | 514 | 1 |
| Bytes | 472,593,990 | 308,312,017 | 6,000,052 |
| JSONL events parsed | 134,777 | 53,451 | Not JSONL |
| JSONL parse failures | 0 | 0 | Not applicable |
| All tool-call occurrences, before dedup | 29,912 | 29,617 | 0 |
| Shell-tool occurrences, before dedup | 19,833 | 13,280 | 0 |
| Deduplicated But-containing shell payloads | 4,146 | 1,761 | 0 |
| Literal/template But fragments in those payloads | 5,665 | 1,958 | 0 |
| But shell session hashes / source files | 410 / 424 | 480 / 480 | 0 |
| Skill launcher / skill-file-read payloads | 224 / 148 | 0 / 883 | 0 |
| All But-related payloads, including reads | 4,518 | 2,644 | 0 |
| Duplicate related call occurrences removed | 67 | 0 | 0 |
| Paired shell payloads | 4,146 | 1,758 | 0 |
| Shell payloads containing help | 196 | 82 | 0 |
| Shell payloads containing `but skill` | 16 | 9 | 0 |

OpenCode's one `.jsonl`-named file is a pretty-printed JSON array of **226 file-diff objects** with `file`, `before`, `after`, `additions`, `deletions`, and `status`. It is not a transcript. Code/examples inside it are not runtime command evidence. Its exact hash and shape are recorded in `manifest.json`; it is excluded from execution denominators.

The primary unit is an actual submitted Bash/bash tool-call payload containing a conservatively recognized But command position. It is **not** a claim that every semicolon/conditional fragment executed. In particular, 3,223 Claude and 398 Pi payloads are flagged as possibly aggregate. Only 907/1,360 respectively have a single non-aggregate candidate and no parser flags; even those require their paired result for outcome claims. We do not divide aggregate failures by literal fragments to produce a command failure rate.

## Corpus composition matters

Tags overlap, and are deliberately not a forced partition. A benchmark tag means the **source file** exposes an explicit `but-bench`, trial-directory, or version-control-bench temporary-run path in relevant working-directory/command evidence. It does not prove every later command in a long file was a benchmark trial. Development context does not mean a command is an intentional negative test: many are ordinary code-review or commit tasks on the GitButler repository.

| Context tag | Claude payloads / session hashes | Pi payloads / session hashes |
|---|---:|---:|
| Explicit benchmark-path source context | 1,336 / 241 | 0 / 0 |
| GitButler product-development cwd | 2,270 / 128 | 1,711 / 460 |
| Explicit `target/debug/but` or `target/release/but` executable | 233 / 26 | 1,406 / 403 |
| Neither source tag nor development executable hint | 637 / 41 | 39 / 16 |
| Benchmark + product-cwd overlap | 101 / 1 | 0 / 0 |

Other/unknown is not proof of production use or a distinct user. The explicit website hunk-ID failure and efficient FAQ-fix cases C01/C12 provide stronger examples than broad unknown-context counts. C06 is a publishing-tool implementation task. Pi is heavily weighted toward GitButler development and read-only review; this explains much of its command mix without requiring a model-behavior explanation.

## Provenance, models, and historical syntax

But shell event timestamps span **2026-05-28–2026-09-04** for Claude and **2026-03-22–2026-08-25** for Pi. These are event timestamps; exported filenames sometimes describe a different session/export date, so filenames are not the authoritative execution dates.

| Observed model metadata | But shell payloads |
|---|---:|
| Claude: `claude-fable-5` | 1,650 |
| Claude: `claude-opus-4-8` | 1,455 |
| Claude: `claude-opus-5` | 1,008 |
| Claude: `claude-sonnet-5` | 31 |
| Claude: `claude-opus-4-7` | 2 |
| Pi: `gpt-5.6-sol` | 1,292 |
| Pi: `gpt-5.5` | 416 |
| Pi: `gpt-5.3-codex` | 48 |
| Pi: `gpt-5.6-luna` | 5 |

There is no Astra or Fable 5.1 evidence in these recognized donated shell calls. Claude's envelope `version` identifies its **agent CLI**, not But; Pi's session `version: 3` is a **file-format version**, not its agent CLI. Pi's models come from model-change/message metadata and are not inferred from the Pi or Claude names.

Actual `but --version` output includes `dev`, `0.21.2`, and multiple `0.5.21xx` builds. Version observations have their own call/result pointers in `supplemental-summary.json`. They are not propagated across sessions or a PATH/binary switch. The normalized per-call `but_cli_version` remains null where no specific binary-version mapping is established.

Historical changes are visible in both directions. C04 shows current-style `commit -b` rejected by an older installed CLI; other calls successfully use the old positional branch/`--changes` syntax. C05 shows a later explicit `rub` retirement notice and successful amendment recovery. C11 has an observed `--anchor` deprecation notice. These are reasons to make current recipes/version mismatch handling legible, **not** to add every old spelling back to the main skill. The parent is checking current implementation semantics independently.

## What the commands show

Largest fragment families are Claude status 1,421, commit 1,119, diff 925, amend 599, push 209, move 192, PR 164; Pi diff 862, show 431, status 373, commit 82, branch 34. `extraction-summary.json` contains the complete family distribution, including dynamic `$@`/loop templates and historical experimental commands. A family occurrence is not a separately proven execution.

Concrete findings with native results:

- **ID shape is a real source of repair loops.** C01 passes hunk suffixes alone, then succeeds with complete file:hunk IDs. C02 passes a comma list as one argument, then succeeds with space-separated IDs. C06 must keep printed disambiguating suffixes. C07 commits the first part of a chain, then refreshes an ID that now names another entity. A single generic “use IDs” sentence needs to convey these distinct facts clearly.
- **Target/source roles deserve a conspicuous recipe.** C03's `amend <commit> <files>` fails and `amend --target <commit> <files>` immediately succeeds. Nine Claude tool-error payloads have the native missing-required-argument marker; the broader marker count is 20 because aggregate success can hide a failed fragment.
- **Git/gh habits leak into read and PR commands.** C08 has native rejections of `diff --stat`, branch-plus-pathspec arguments, and two diff targets. C10 has `pr new --title/--body`, a failed literal `-F -`, then a successful real message file. These support concise current recipes/cardinality guidance instead of an exhaustive API listing in the main skill.
- **Some delays belong to the environment or CLI.** Pi has 46 command-not-found-marker payloads, 42 also reported as tool errors. C09 recovers with an existing build-output path. Setup-required, locking, atomic dependency rejection, and CLI bugs also appear; a model-only explanation would be wrong.
- **Trusting the concise mutation result is viable.** C12 shows `but diff` → `but commit -b ... <file-id>` creating the branch/commit successfully. C05 does not execute its help fallback after success. C10's successful `pr new` already pushes. These support the existing fast-path concept without proving it is optimal for every task.

All 25 recognized `but skill` payloads are installation/check commands (Claude: 8 install, 8 check/update; Pi: 8 install, 1 check/update). There are **zero recognized `but skill --full`, `but skill reference`, `but skill examples`, or bare content-loading calls**. This corpus therefore cannot evaluate the new stub's on-demand content loading. A Claude Skill result often only says `Launching skill: gitbutler`; a launcher call is not proof of subsequent guide exposure or adoption. Skill-file reads include product documentation inspection, not just loading instructions to follow.

## Error counts, retries, and apparent extra checking

| Payload-level marker | Claude | Pi |
|---|---:|---:|
| Tool reports an error | 128 | 126 |
| Broad error-like word cue | 488 | 349 |
| Both | 100 | 109 |
| Word cue without tool-reported error | 388 | 240 |
| Anchored native syntax marker + tool error | 31 | 47 |
| Native ID-not-found/ambiguity marker + tool error | 13 | 13 |

Even the native-marker rows are **candidates**, not a model-mistake rate: development tasks deliberately probe invalid syntax, and aggregate output may contain another process's output. Fourteen detailed cases use direct paired error/success evidence rather than a keyword label.

To check the broad-word heuristic, a fixed seed (910) sampled ten cue-only/nonerror payloads per harness. Manual inspection found **6/10 Claude and 9/10 Pi were false positives if labelled But execution failures**: source diff text, successful `0 failed` test summaries, or errors from another command. The other 4/10 and 1/10 contained real But errors whose aggregate tool result was nonerror; some were intentional negative probes. This is a conditional validation sample, **not** an estimated whole-corpus false-positive or task-failure rate. Exact sample keys and judgements are in `keyword-validation.json`.

Sequence scan finds 625 Claude / 35 Pi occurrences where the next But payload after a mutation is an inspection, and 4 / 82 where the next But inspection has identical shell input. These are **not redundant-check counts**: other edits, tests, instructions, or mutations can occur between those But calls. Most inspected repeated Pi diffs have changed output. C13 supplies a narrow counterexample where two consecutive calls repeat the same wrong-context status and receive identical setup-required output. Do not turn the full sequence counts into wasted-time estimates. Same-family nonerror-after-error candidates number 71/57, also not confirmed repaired retries without context.

Question tools total 37 Claude / 87 Pi occurrences. The word-match candidate list contains ordinary product-design questions and English “but”; it is explicitly not a permission-ceremony metric. Manually seen publication questions include a force-push to another contributor's PR branch (Claude file `2026-07-25T11-16-23_44447…`, L209), already-pushed branches about to be squashed (`2026-08-07T13-12-26_05b73…`, L129), and whether to push local CI fixes (`2026-08-24T12-28-02_bff3bd…`, L125/L354). Pi asks for missing branch/commit names (`2026-07-24T21-27-23_477265…`, L8) and resolves a main/master mismatch (`2026-08-06T15-29-29_bc89a…`, L20). These do not establish unnecessary permission-taking caused by the main skill. No broad permission-overhead claim is supported.

## Reproduction and limitations

Run `python3 extract.py`, then `python3 supplement.py`, then `python3 build-cases.py` in this directory. Source roots and output roots are constants; sources are opened read-only. No CLI/model/benchmark trial is launched. `manifest.json` records every original file's SHA-256, bytes, format, counts, and the extractor hash.

`normalized-calls.jsonl` is the detailed private audit index; CSV is the flat review view. Source pointer, call ID, event ID, model/date, sanitized fragments, result pointer/hash/excerpt, and all duplicate occurrences are retained. Claude shell IDs were all `toolu_` strings of length 30; Pi IDs were all length-83 `call_…|fc_…` strings. Dedup is exact provider+stable call ID+tool+input, not same command text. Missing IDs would fall back to an event ID then a file/line pointer. All 67 removed duplicate related occurrences are Claude; they remain linked in occurrences. We do not claim dedup of all non-But tools or that distinct IDs represent distinct users.

Parser limits are intentional: it does not execute/evaluate shell text; heredoc bodies and interpreter-source strings are excluded; command lookup (`command -v but`) is excluded; shell `-c` wrappers and local binary paths are recognized. Dynamic arguments, substitutions/backticks, function-definition payloads, and other uncertainty have explicit flags. Redirections and separators are simplified when reconstructing argv, so exact command semantics should be read from original source pointers. Only the first 2,000 lexical-But-but-no-recognized-invocation candidates are indexed (all-corpus occurrence counts are 1,462 Claude / 1,903 Pi after lookup exclusion); this bounded candidate list is not an exhaustive missed-command audit. “But” in arbitrary application code, quoted guidance, test fixture strings, and OpenCode diffs is not automatically an execution.

There is no native process lifecycle stream to prove every nested segment or attach timing to it. Three Pi shell calls lack paired results; none are treated as successful. Truncated excerpts and aggregate results limit automated diagnosis. Counts from this file must not be merged with current Hermes benchmark runs as if they were one experiment. Files, sessions, users, and models are different units.

No original traces, repositories, runtime control, fixtures, benchmark inputs, scores, or optimization state were changed. Derived excerpts omit unrelated chats/system prompts and redact common credential forms and unnecessary path identities; source filenames and stable event/call IDs remain solely for local provenance.

- Publication/missing-information question source: [claude/2026-07-25T11-16-23_44447cca17963b76066400cc7dda711b79056e41b9d955dd277e0713973de1cc.jsonl L209](/Users/kiril/tmp/traces/claude/2026-07-25T11-16-23_44447cca17963b76066400cc7dda711b79056e41b9d955dd277e0713973de1cc.jsonl:209), pointer `/message/content/0`.

- Publication/missing-information question source: [claude/2026-08-07T13-12-26_05b73c93190dac6379df0f63c0e350efd466676788d66763c632cedbd2a12dbc.jsonl L129](/Users/kiril/tmp/traces/claude/2026-08-07T13-12-26_05b73c93190dac6379df0f63c0e350efd466676788d66763c632cedbd2a12dbc.jsonl:129), pointer `/message/content/0`.

- Publication/missing-information question source: [claude/2026-08-24T12-28-02_bff3bd28f6465d8ea7913a22fe5bf55ad56b558e6317df7f4d42eec71d3f3d34.jsonl L125](/Users/kiril/tmp/traces/claude/2026-08-24T12-28-02_bff3bd28f6465d8ea7913a22fe5bf55ad56b558e6317df7f4d42eec71d3f3d34.jsonl:125), pointer `/message/content/0`.

- Publication/missing-information question source: [claude/2026-08-24T12-28-02_bff3bd28f6465d8ea7913a22fe5bf55ad56b558e6317df7f4d42eec71d3f3d34.jsonl L354](/Users/kiril/tmp/traces/claude/2026-08-24T12-28-02_bff3bd28f6465d8ea7913a22fe5bf55ad56b558e6317df7f4d42eec71d3f3d34.jsonl:354), pointer `/message/content/0`.

- Publication/missing-information question source: [pi/2026-07-24T21-27-23_477265ec162f5e6787f2292bdfd85ebdb3394aee73ac459e831d0758de467931.jsonl L8](/Users/kiril/tmp/traces/pi/2026-07-24T21-27-23_477265ec162f5e6787f2292bdfd85ebdb3394aee73ac459e831d0758de467931.jsonl:8), pointer `/message/content/1`.

- Publication/missing-information question source: [pi/2026-08-06T15-29-29_bc89a70f74953e75685408782c38868dcf1c871fb957f2ca9d103584920917c4.jsonl L20](/Users/kiril/tmp/traces/pi/2026-08-06T15-29-29_bc89a70f74953e75685408782c38868dcf1c871fb957f2ca9d103584920917c4.jsonl:20), pointer `/message/content/1`.
