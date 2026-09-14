# Donated Codex: manually reviewed evidence cases

Reviewed 10 purposively selected windows across 10 source files: early/later syntax errors, ID resolution, a version transition, efficient commits, post-result inspection, help discovery, and a mixed-output negative control. This is stratified qualitative review, not a random prevalence sample. Only visible tool submissions/results were used for the observations below. Branch names/messages/paths are redacted; source pointers and call IDs remain exact.

## C01 — Git-style flags and multi-target diff need correction

`but status --short` returns exit 2 at line 169: “unexpected argument '--short' found”; plain `but status` succeeds at 179. A two-path `but diff` then returns exit 2 at 187; bare `but diff` succeeds at 195.

High confidence in the rejected syntax and recovery. This supports a concrete command map, especially diff cardinality. It does not show the current skill caused the mistake or measure present-day failure rates.

Evidence: [167](/Users/kiril/tmp/traces/codex/2026-02-21T21-44-17_51eae6c4c7f88b8d65fe9218dadb30300dd2b7d599f944ae50ddc7deae6059b3.jsonl:167) (`call_wd4lNa1BCJ4lofFOgI1PkrTR`); [177](/Users/kiril/tmp/traces/codex/2026-02-21T21-44-17_51eae6c4c7f88b8d65fe9218dadb30300dd2b7d599f944ae50ddc7deae6059b3.jsonl:177) (`call_525YltuS1CZ6RFwr5ooNuKwy`); [185](/Users/kiril/tmp/traces/codex/2026-02-21T21-44-17_51eae6c4c7f88b8d65fe9218dadb30300dd2b7d599f944ae50ddc7deae6059b3.jsonl:185) (`call_zbCQDzzUqQXXhf6b6pcs4s10`); [193](/Users/kiril/tmp/traces/codex/2026-02-21T21-44-17_51eae6c4c7f88b8d65fe9218dadb30300dd2b7d599f944ae50ddc7deae6059b3.jsonl:193) (`call_i1Oan1fmOW5ROU0aZwefxBaP`).

## C02 — The multi-ID diff mistake persists in a later version

After `but status -fv`, the agent submits `but diff sy ty ut rv`. Line 211 rejects `ty` with exit 2 and `Usage: but diff [OPTIONS] [TARGET]`. Four separate per-ID diff calls follow; their outputs at 220–223 report exit 0. One is truncated (68,978 original tokens).

High-confidence cardinality friction, recovered without inventing new IDs. Splitting the calls solves syntax, but does not guarantee readable output volume. The truncation is observed; any later time cost is unmeasured.

Evidence: [202](/Users/kiril/tmp/traces/codex/2026-07-30T20-28-44_d251fe5532510066706cf18e31cac6bc1201d6f221e7b520ead44ef7f1d52efc.jsonl:202) (`call_zEuXor4ooZ9SbMmmtzY1NIJf`); [210](/Users/kiril/tmp/traces/codex/2026-07-30T20-28-44_d251fe5532510066706cf18e31cac6bc1201d6f221e7b520ead44ef7f1d52efc.jsonl:210) (`call_rCrSTEFykFMnBwhRULEVJ70R`); [216](/Users/kiril/tmp/traces/codex/2026-07-30T20-28-44_d251fe5532510066706cf18e31cac6bc1201d6f221e7b520ead44ef7f1d52efc.jsonl:216) (`call_WLygBWZEl0XZreYC4BReI0rl`); [217](/Users/kiril/tmp/traces/codex/2026-07-30T20-28-44_d251fe5532510066706cf18e31cac6bc1201d6f221e7b520ead44ef7f1d52efc.jsonl:217) (`call_BsnAFBj9J8hIi7eFVn0CD0VB`); [218](/Users/kiril/tmp/traces/codex/2026-07-30T20-28-44_d251fe5532510066706cf18e31cac6bc1201d6f221e7b520ead44ef7f1d52efc.jsonl:218) (`call_hPljqEnoqOvgo8mM6JWFaxIl`); [219](/Users/kiril/tmp/traces/codex/2026-07-30T20-28-44_d251fe5532510066706cf18e31cac6bc1201d6f221e7b520ead44ef7f1d52efc.jsonl:219) (`call_sHvEzchMzaW7Feriyn5rVVgh`).

## C03 — Missing IDs are real; historical commas are not the error

The historical form `but commit od -m <message> --changes nz,pw --json --status-after` fails at 1111: `Invalid file ID(s)`, then separate messages that `nz` and `pw` were not found. The agent rereads status at 1118.

High confidence in failed ID resolution; origin of the bad IDs remains unknown. The old CLI split the comma list correctly. This case must not be presented as proof that comma separation was invalid in February, or that the agent invented the IDs.

Evidence: [1102](/Users/kiril/tmp/traces/codex/2026-02-12T08-29-26_ec59c640c6e7bc0eb378ef3462d35e563c6cef2eb961c3c4d3e275331dbe10be.jsonl:1102) (`call_UVDvwf6zKgHTO1ngeIlytedN`); [1109](/Users/kiril/tmp/traces/codex/2026-02-12T08-29-26_ec59c640c6e7bc0eb378ef3462d35e563c6cef2eb961c3c4d3e275331dbe10be.jsonl:1109) (`call_qqUaQqnBEPhNBblyKJkmWSjY`); [1118](/Users/kiril/tmp/traces/codex/2026-02-12T08-29-26_ec59c640c6e7bc0eb378ef3462d35e563c6cef2eb961c3c4d3e275331dbe10be.jsonl:1118) (`call_1lXdY0h4GqvH5H8XRGO5NRhJ`).

## C04 — A cached old commit form is corrected to the new form

`but commit <branch> -c -m <message> --changes rm,uk,km` is rejected at 4583: unexpected `-c`, with the positional `[CHANGES]...` usage. The next submission is `but commit -b <branch> -m <message> rm uk km`; the continuation result at 4591 says a commit was created.

High-confidence historical syntax transition and successful local repair. The continuation has a different call ID, illustrating why initial-response pairing alone cannot yield a reliable success rate. The corpus contains extensive valid earlier use of `--changes`; do not label all of it misuse.

Evidence: [4582](/Users/kiril/tmp/traces/codex/2026-07-27T22-02-04_3953250309515a2e3550d06059c569f6c2153d8992ccbe54c010cc7db3157051.jsonl:4582) (`call_88UYgRWPIe2mFEiPMkdcwlNO`); [4586](/Users/kiril/tmp/traces/codex/2026-07-27T22-02-04_3953250309515a2e3550d06059c569f6c2153d8992ccbe54c010cc7db3157051.jsonl:4586) (`call_nAaAFPpj8GuEC61yzzBkHVdz`); [4591](/Users/kiril/tmp/traces/codex/2026-07-27T22-02-04_3953250309515a2e3550d06059c569f6c2153d8992ccbe54c010cc7db3157051.jsonl:4591) (`call_XRxDXl0wh7h9tHBvdrNGSlpS`).

## C05 — A compact successful selected-change commit exists

`but diff` is followed by `but commit -b <branch> -m <message> zl zs sm`. The paired output at 109 confirms `Created commit qwz on new branch <redacted>`. There is no separate branch-creation command in this local sequence.

The intended two-command selected-change recipe is demonstrably usable. This is a bounded CLI sequence, not a claim that the entire development task required only two tools or that all later inspection would be unnecessary.

Evidence: [101](/Users/kiril/tmp/traces/codex/2026-08-22T02-32-39_d6f734c697a038f5f22eccabbb73f71ee287a75aef4ce07bc88a7415f03600e8.jsonl:101) (`call_B8pvzNkvyJq4SIMqEwFgVX0i`); [107](/Users/kiril/tmp/traces/codex/2026-08-22T02-32-39_d6f734c697a038f5f22eccabbb73f71ee287a75aef4ce07bc88a7415f03600e8.jsonl:107) (`call_ENUgIq4lqNwvCWpf4aCeTxSF`).

## C06 — Post-commit show is observable, redundancy is not automatic

`but diff` → targeted `but commit -b <branch> -m <message> yo`; output at 329 confirms newly created commit `knm`. The next retained CLI call is `but show knm`.

This is evidence for post-result inspection, and correct reuse of a returned ID. A stopping rule could remove it only if the task needs no contents/detail beyond the creation confirmation. The trace pattern alone does not establish wasted work.

Evidence: [321](/Users/kiril/tmp/traces/codex/2026-08-26T13-09-23_e188467891d61fca18025be5eca700faf007b5067f228fabb53ab326f77413de.jsonl:321) (`call_xUm2t808M7P4sA0zVCaCfN9m`); [327](/Users/kiril/tmp/traces/codex/2026-08-26T13-09-23_e188467891d61fca18025be5eca700faf007b5067f228fabb53ab326f77413de.jsonl:327) (`call_zY3GXOQgODkUeN26AAVpsZCs`); [333](/Users/kiril/tmp/traces/codex/2026-08-26T13-09-23_e188467891d61fca18025be5eca700faf007b5067f228fabb53ab326f77413de.jsonl:333) (`call_MhF4O8eiZZ2vVsAt3vKQ1r2q`).

## C07 — A second returned-ID inspection pattern

After a bare diff, a targeted five-ID commit succeeds at 492 and returns `wpl`; `but show wpl` follows at 496.

The pattern recurs across distinct files, but that is not evidence of independent users. It reinforces a narrowly scoped “stop when the result already supplies the needed fact” rule, not a ban on verification.

Evidence: [484](/Users/kiril/tmp/traces/codex/2026-08-27T02-34-05_f95595ec91fba4eb84e8556e28a06eac11243f9f597bfbfe5c8830ca63e379ff.jsonl:484) (`call_JxOMEiFLZFO82L6AkyOD7ZuI`); [490](/Users/kiril/tmp/traces/codex/2026-08-27T02-34-05_f95595ec91fba4eb84e8556e28a06eac11243f9f597bfbfe5c8830ca63e379ff.jsonl:490) (`call_dc8S4YtEx21Io3iIqfarFB11`); [496](/Users/kiril/tmp/traces/codex/2026-08-27T02-34-05_f95595ec91fba4eb84e8556e28a06eac11243f9f597bfbfe5c8830ca63e379ff.jsonl:496) (`call_mLh2znOWPFVlc2pxscsrCzTw`).

## C08 — Source-code output is not a retired-command failure

The submitted mixed shell batch reads Rust source about retired syntax and also runs `but --version`. Output at 416 contains that source and the actual version line `but 0.5.2152`. The source mentions removed `rub` and old commit forms.

Negative control for the miner: keyword matches in this result are not executed `but rub` commands or CLI failures. Mixed batches must not attribute every output error word or one exit code to the parsed But segment.

Evidence: [415](/Users/kiril/tmp/traces/codex/2026-08-14T19-44-09_b470fd2e60491b1e29826579e900a445a838b171926005b1e64fb5efc79f3a1f.jsonl:415) (`call_ITHdgmS7QNhNxZYUY9r9iEed`).

## C09 — The placement of help matters

`but help squash` fails at 3652 with unexpected `squash`; `but squash --help` succeeds at 3661. `but push --help` follows and succeeds at 3668.

High-confidence discovery syntax friction. The second help call may serve a different necessary operation; a sequence of help calls is not by itself an unnecessary help loop.

Evidence: [3650](/Users/kiril/tmp/traces/codex/2026-02-07T17-37-09_44ca2c9e154d0111c8850af70624df36eae6671dbda2475adf94e8afc8bc8c69.jsonl:3650) (`call_Yzt2OgUEuHRGFDv8j8PfA2tz`); [3659](/Users/kiril/tmp/traces/codex/2026-02-07T17-37-09_44ca2c9e154d0111c8850af70624df36eae6671dbda2475adf94e8afc8bc8c69.jsonl:3659) (`call_kcmFLCdHhdqK1LjhkEBtzjLf`); [3666](/Users/kiril/tmp/traces/codex/2026-02-07T17-37-09_44ca2c9e154d0111c8850af70624df36eae6671dbda2475adf94e8afc8bc8c69.jsonl:3666) (`call_MO8hV8KjU1BWWnRBT2f5dKwr`).

## C10 — Discovery probes can be intentional negative tests

This development-context window tries `but help link`, `but link help`, `but link foo`, and bare `but link`; the results at 78, 79, 86 and 93 reject the forms or require a subcommand.

The deliberately generic `foo` and repeated missing/help forms look like parser/discovery probing, but intent is not automatically recoverable. These are real rejected submissions, not necessarily ordinary workflow mistakes. Keep this distinction when using the 223-batch diagnostic subset.

Evidence: [75](/Users/kiril/tmp/traces/codex/2026-03-04T14-01-53_170fae60ac65c94c44a0ff1f226ba7f5d4d7612cf6e24abbc32102e38e56ae00.jsonl:75) (`call_2yHJ2GJGdpcpzopR7nF3nNpo`); [76](/Users/kiril/tmp/traces/codex/2026-03-04T14-01-53_170fae60ac65c94c44a0ff1f226ba7f5d4d7612cf6e24abbc32102e38e56ae00.jsonl:76) (`call_CqIOHDLRL1HpfyHKTRMEVuah`); [84](/Users/kiril/tmp/traces/codex/2026-03-04T14-01-53_170fae60ac65c94c44a0ff1f226ba7f5d4d7612cf6e24abbc32102e38e56ae00.jsonl:84) (`call_Wx3YRaox1KTuICzLySrnnAjK`); [91](/Users/kiril/tmp/traces/codex/2026-03-04T14-01-53_170fae60ac65c94c44a0ff1f226ba7f5d4d7612cf6e24abbc32102e38e56ae00.jsonl:91) (`call_2VIpcXUL2I3GTWqyw5CfnLS4`).
