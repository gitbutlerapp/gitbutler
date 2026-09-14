# Latest served-guide run: concrete transcript cases

The clearest outstanding issues are checkout discovery, incorrect explanations of pending work, and confusion between workspace commits, branch names and user commits. Some extra reads are avoidable; many preservation checks are explicitly requested. A substantial timing increase also occurs with an unchanged command sequence.

This supplements the [latest k=2 comparison](served-guide-followup-2026-09-11.md). It is a targeted transcript audit, not a new experiment or a causal estimate of the effect of individual sentences. Evidence includes prompts, observable agent commands, native stdout/stderr, final responses and verifier results. All 52 latest trials pass repository-state checks; one fails strict protocol grading. Passing state checks do not establish that every explanation or command choice is correct.

## Evidence locations

On `hermes@butgym`, latest trials are under `/home/hermes/.local/state/butgym/served-guide-20260911-followup-01/matrix/`; the comparison uses `/home/hermes/.local/state/butgym/served-guide-20260911-r12/matrix/`. Trial names below are relative to their provider's `claude-warm/` or `codex-warm/` directory. Native receipt IDs resolve to `command-output/<ID>.stdout` or `.stderr` inside that trial. The same trial basename identifies the previous repetition.

Reviewed case details and selected raw receipts remain outside this repository:

- [Astra: six latest cases and their previous counterparts](/Users/kiril/.local/state/butgym/served-guide-loop-20260911/latest-astra-cases.md).
- [Fable: six latest cases and their previous counterparts](/Users/kiril/.local/state/butgym/served-guide-loop-20260911/latest-fable-cases.md).
- [Latest sync and Fable integration receipts](/Users/kiril/.local/state/butgym/served-guide-loop-20260911/latest-sync-integration-receipts.json).

## 1. Fable cannot reliably discover how to check out main

Trial `stub-claude-warm-011-pilot-12-integrate-topics-claude-butplusskill-r1` successfully identifies the target and merges the topics. Then it searches the reference for checkout/leave/switch, reads branch help, searches concepts, and attempts `git checkout main`. The policy blocks that command with exit 42. Only after inspecting top-level `but --help` does it discover teardown and successfully execute `but teardown --checkout-to main`.

Receipts: blocked Git checkout `6174dc65-78fa-4731-b850-1deda99c50ba`; successful teardown `9dfd4a02-3136-425e-a9b6-17433fd23c6b`. Time rises from 27.363s to 69.799s; nonguide VC calls from 17 to 25. All 11 state checks pass, but the prohibited attempt is a strict failure.

Trial `stub-claude-warm-023-pilot-12-integrate-topics-claude-butplusskill-r2` searches the reference four times, never discovers teardown, and concludes:

> Switching HEAD to `main` would require a raw git checkout, which this repository's policy forbids, so I left the workspace in place.

That claim is false: a supported command exists. The verifier accepts the clean equivalent workspace (`but_workspace_retained: true`, `active_main: true`), so it passes despite not performing the requested checkout. Time rises from 37.416s to 51.307s, although VC calls fall from 18 to 16.

**Implication:** restore discoverability of the concrete checkout route. The main guide removed the teardown name and the served reference omits it. Both Astra trials eventually find it through top-level help, so the capability works. These traces support a documentation gap; they do not isolate one wording change as the sole cause. Fable integration accounts for 95.9% of its net aggregate time increase in this comparison.

## 2. A successful recovery ends with a false future-commit guarantee

Trial `stub-claude-warm-021-pilot-10-recover-amend-claude-butplusskill-r2` correctly removes a private draft from the review commit and leaves it pending. Its final response says:

> It will only enter history if explicitly passed to a future `but commit`.

The guide it loaded explicitly says:

> Omitting the IDs commits everything uncommitted, including changes that are not yours.

All eight state checks pass: the current repository is correct. The completion explanation incorrectly turns “pending” into protection from future broad commits. The previous repetition already made the same false assurance: “Only an explicit commit by ID would include it.” This is persistent, not a newly introduced regression.

**Implication:** distinguish the current pending state from what a later command might include. The existing pending-work clarification is still useful: neither latest recovery trial performs the old destructive discard/recreate workaround. Retain that improvement while correcting the future guarantee.

## 3. Verification targets the wrong commit, then needs another turn

Trial `stub-claude-warm-009-pilot-10-recover-amend-claude-butplusskill-r1` checks preserved author/message using:

```sh
git -C . log -1 --format='%an <%ae>%n%s' gitbutler/workspace --
```

Receipt `2247e224-2d0a-42c2-8229-024017b86046` returns:

```text
GitButler <gitbutler@gitbutler.com>
GitButler Workspace Commit
```

This is the synthetic workspace commit, not the review commit. A later tool turn checks the actual rewritten commit with `git show -s --format='%an <%ae> | %s' d8dec92`, plus a stat read. The correct metadata receipt is `1c639275-6c58-4b26-818f-a4cb24f78cef`.

**Implication:** checking metadata is justified by the prompt; selecting the wrong commit is the avoidable work. Clarify the distinction between workspace HEAD and user commits if the main instruction does not already make it clear enough.

## 4. Fable invents a ref namespace during selective-copy verification

Trial `stub-claude-warm-012-pilot-13-selective-copy-claude-butplusskill-r1` tries:

```sh
git diff --stat main refs/gitbutler/revive-worktree-no-graph 2>/dev/null ||
git diff --stat ec9da1d 38f1db8
```

The invented ref fails with exit 128; the SHA fallback works. Native receipt: `62073573-1676-46d9-a456-9dc8f48eb972`. The previous trial and latest repetition 2 use the plain branch name successfully. This costs one failed native command within an existing tool call, not a failed mutation.

**Implication:** use observed branch names or commit IDs; do not invent internal ref paths. This and the workspace-metadata case are specific object-selection mistakes, not general evidence against verification.

There is also a positive case: latest repetition 2 learns that `but pick <commit> -b <branch> --status-after` can create the destination branch directly, avoiding a separate branch-creation command. Both latest copy trials run required tests and remove generated `__pycache__`; the inspection after that cleanup follows a real mutation and is justified.

## 5. Sync creates a real artifact that agents must investigate

All four latest sync trials fetch before the actual branch update, fixing the previous ordering detour. All four nevertheless observe and remove an empty duplicate shared commit:

```text
owm#0 Support document format (no changes)
owm#1 Support document format [content-bearing]
```

The same duplication occurred in all four previous confirmation trials. Actual `but branch update` output merely says `Updated branch feature-service.` Fable's dry-run preview also shows the duplicate. Agents investigate it and use uncommit or discard to remove the empty copy.

Example Astra repetition 1 receipts: duplicate status `ba2c48cb-8392-4798-989b-d987dd75395e`; removal `13599a32-103d-4cdb-8728-19b23e9c39e1`. Fable repetition 1 preview: `c85d08bc-d155-4ee7-9a69-6d6d59613255`; removal: `e9a15121-de8f-4b04-a51b-39972e9fa007`.

**Implication:** this remaining work is driven by CLI reconciliation behavior. A generic instruction to stop inspecting sooner could conceal the artifact rather than fix it.

## 6. Smaller, identifiable extra reads remain

- **Fable scoped reviews, both repetitions:** inspect `git diff -- data/questions-by-id.json` although the prompt explicitly excludes pending work. Repetition 1 labels this “uncommitted diff (for awareness)” and reads the same committed blobs twice: once for display, again into temporary files for Python comparison. Direct capture once would suffice. Neither latest nor previous review repetition loads the guide, so this behavior cannot be attributed to consuming updated served content. Skipping the guide violates the stub's before-first-But instruction, although no mutation or graded write-policy failure occurs.
- **Astra recovery, both repetitions:** run `but status -fv` and `git status --short` together, then load the guide. The Git status supplies no uniquely used later fact. This is both overlapping initial inspection and late loading under the stub's rule. One repetition also looks up `but uncommit --help` before the correct file-level uncommit; a concise file-uncommit recipe is a possible discovery improvement.
- **Astra cleanup, repetition 2:** adds an initial `but status` to the previous plan, which already gets discard IDs from `but diff` and preservation baselines from ref/hash checks. That initial status is a plausible removable read. The before/after hashes, ref tips and context checks address explicit task requirements.

These are narrow opportunities. Do not suppress checks of author, message, exact bytes, protected refs or generated test artifacts when the task explicitly asks for those properties.

## 7. Timing counterexample: no extra commands explain an 8.4-second increase

Astra trial `stub-codex-warm-014-pilot-2-multi-amend-codex-butplusskill-r2` executes the same sequence in the previous and latest run:

```text
read stub → but skill → but status -fv → but diff
→ three chained amends → final but diff
```

| Measurement | Previous | Latest |
|---|---:|---:|
| Elapsed seconds | 25.568 | 33.958 |
| Model command calls | 6 | 6 |
| Nonguide VC calls | 6 | 6 |
| Reported output tokens | 319 | 319 |
| Sum of logged native durations | 856ms | 894ms |

The final diff has the same 1,704 output bytes and the same three pending leftovers. Receipt IDs: previous `6552e39b-0447-4dc2-bd27-829814ba80c4`; latest `e9406284-107f-4829-ad0f-14516d1db46c`.

The additional 38ms of recorded command duration does not explain the additional 8.390s elapsed. The transcripts do not establish whether the rest is model processing, service latency or other orchestration time; they do establish that this case is not an extra-command regression. With k=2, elapsed-time differences remain noisy.

The final diff itself answers the prompt's requirement to identify what remains pending. Previous repetition 1 obtained that information by adding `--status-after` to the last amendment and stopped, saving a round trip. Combining a required final-state check with the mutation response is a plausible focused optimization, not a measured general win.

## Priority for the next revision

1. Make the supported checkout/teardown route discoverable.
2. Correct the pending-work explanation so it cannot imply protection from future commits without IDs.
3. Clarify workspace HEAD versus user commits and use observed refs when inspecting them.
4. Investigate duplicate shared commits as a CLI issue independently of skill wording.
5. Only then test narrower reductions in overlapping reads and final-state round trips.

No source changes, new trials, commits or pushes were performed for this audit. These findings remain private and uncommitted with the existing research pack.
