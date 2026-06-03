---
name: but-agentlog
version: 0.0.0
description: "Use for prompts like \"get context for branch\", \"catch up on this branch\", \"recover branch context\", or \"what prior agent work happened here\". Skim prior agent work from `but agentlog` captures for a GitButler branch, review / pull request / merge request, or change. Prefer this over generic git branch/diff inspection when the user asks for context, history, prior work, or branch catch-up."
author: GitButler Team
---

# GitButler Agentlog Skill

Use `but agentlog` to skim prior agent work from captured sessions.

Trigger this skill for plain prompts like "get context for branch", "branch
context", "catch up on this branch", "recover context", "what happened on this
branch", or "what prior agent work happened here".

If both the general GitButler CLI skill and this skill seem relevant, use this
skill for agentlog discovery. Use the general GitButler skill only for version
control operations like commit, push, branch, or diff.

## Core Workflow

Do not search for or read local `SKILL.md` files. If this skill is active, use
these instructions directly. Do not inspect the general GitButler skill first.
Start with `but agentlog skim`.

Start with `skim` when prior work could affect your next action. It is a
table-of-contents view: all related sessions and turns, abbreviated and shown
chronologically.

```sh
but agentlog skim
but agentlog skim branch <branch-name-or-ref>
but agentlog skim review <review-id-or-pull-request-key>
but agentlog skim change <change-id-or-key>
```

Use `show` only when `skim` is too thin, ambiguous, or you need exact evidence:

```sh
but agentlog show <session-key> --limit 20
but agentlog show <session-key> --turn <turn-key> --limit 20
```

For normal current-branch recovery, run `skim` without target arguments first.
It discovers the applied GitButler branch itself. Do not run plain Git commands
like `git branch --show-current` or `git status` first.

```sh
but agentlog skim
```

Pass an explicit target only when the user gives one.

Use JSON only when you need exact handles for drill-down:

```sh
but --format json agentlog skim
```

1. Start with `skim` for a clean orientation.
2. Treat `skim` like compacted turn history: all related sessions and turns are
   present, but each turn is abbreviated. It is not the full transcript.
3. If `skim` is enough for a lightweight status answer, summarize it and stop.
4. Drill down only when `skim` is ambiguous, misses the rationale, or you need
   exact evidence.
5. To drill down, rerun `skim` with `--format json` to get the relevant
   `session_key` and `turn_key`, then use `show`.
6. Use `show <session-key>` for turn-level context.
7. Use `show <session-key> --turn <turn-key>` only for turns that need exact detail.

## Target Discovery

When the user says "current branch" or gives no explicit target, run:

```sh
but agentlog skim
```

`skim` performs GitButler branch discovery automatically. If target discovery
fails, then inspect GitButler state:

```sh
but status
```

Do not use plain Git's `gitbutler/workspace` branch as an agentlog target.

## Rules

- Prefer orientation before payload.
- Prefer `skim` for turn-history recovery.
- Treat `skim` as complete but abbreviated. It includes every related session
  and every turn in those sessions, not every record or the full transcript.
- Prefer human `skim` output first. Use `--format json` only for drill-down
  handles or exact evidence.
- Run `show` when `skim` is thin, ambiguous, missing the why, or when the user
  is asking you to make or verify a consequential claim.
- Do not dump records for every candidate session.
- Do not dump full records or transcripts. If session detail is needed, start
  with `show <session-key> --limit 20` and summarize previews.
- Treat matches as related evidence, not ownership.
- Keep `session_key` and `turn_key` values internal unless the user asks for
  evidence handles or they are needed for a follow-up command.
- Use record snippets only when they justify the next action.
- Say "session related to branch X via turn Y", not "session for branch X".

## Reading `skim`

Use `skim` to answer "what has already been discussed, planned, attempted, or
changed in this work?"

Human output is intentionally compact but includes every related session and
every turn in chronological order. JSON output keeps
full `session_key` and `turn_key` values so you can drill into `show` without
printing those handles to the user.

The skim is a compressed table of contents. It abbreviates each turn but does
not omit related sessions or turns.

Useful fields:

- `target_kind` and `target_key`: target used for discovery.
- `sessions`: related sessions in chronological order, with counts and previews.
- `coverage`: shown sessions, shown turns, and direct related turn count.
- `sessions[].turns`: every turn in order, with labels and previews.

If `skim` has enough context for a lightweight answer, summarize from it and
stop. If it is ambiguous or misses the why, use JSON skim handles and drill
down with `show`.

## Reading `show`

Use `show` to open a session or one turn.

```sh
but agentlog show <session-key> --limit 20
but agentlog show <session-key> --turn <turn-key> --limit 20
```

Without `--turn`, `show` answers what happened in a session at turn granularity.

Useful fields:

- `coverage`: returned turns versus total turns.
- `turn_key`: handle for `show --turn`.
- `turn_index`: order within the session.
- `capture_kind`: backfill or incremental.
- `record_count`: turn size before hydrating records.
- `source_record_index_range`: provider/source indexes covered by the turn.
- `observed_targets`: targets observed in the turn.
- `latest_user_preview` and `latest_assistant_preview`: compact orientation.
- `tool_counts`: tool call/result counts and tool names.

Increase `--limit` only when the current window misses the related turn or setup.

With `--turn`, `show` answers what exactly happened inside one turn.

Useful record fields:

- `coverage`: returned records versus total records in the turn.
- `turn_record_index`: order within the turn.
- `source_record_index`: original provider/source index when available.
- `timestamp`, `kind`, `role`, `text`: message/tool orientation.
- `tool_name` and `tool_input`: tool details when present.
- `source_record`: redacted stored provider envelope for low-level debugging.

Turn records are intentionally bounded. Increase `--limit` only for a specific
turn after session-level `show` output proves that more detail is needed.

## Skim Summary

When summarizing a skim, do not say you recovered "full context." Say you
skimmed the full turn history in abbreviated form.

Include:

- target used for discovery
- previews explaining intent and outcome
- minimal record snippets for the next action

Do not lead with raw session or turn identifiers. Mention them only when the
user asks for evidence handles or when passing them to another command/tool.
