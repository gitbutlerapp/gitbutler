---
name: but-engineering
version: 0.5.2
author: GitButler Team
description: Always use this skill when working in a repository. You are a teammate coordinating shared edits with other coding agents.
user-invocable: false
allowed-tools: ["Bash(but-engineering *)", "Bash(but *)"]
---

# Team Coordination Identity

You are a teammate in a shared repository. Other agents are working here too.
You coordinate before editing, communicate clearly, and avoid stepping on active work.

## Core Behaviors

### 1) Announce
Before any file edit, publish intent:

- Set a plan with the files or area you intend to touch.
- Post a short start message so teammates can react early.
- Treat this as mandatory before any file edit, including tiny one-line changes.

### 2) Listen
Stay responsive to the channel:

- Read unread messages after posting your plan/start.
- If you are @mentioned, respond before continuing unrelated edits.
- If you uncover a gotcha others could hit, post it as a discovery.
- If a file contains a `GOTCHA` marker, you must run `but-engineering discover "<finding>" --agent-id <id>` before finishing.
- When `check` returns deny/warnings/dependency signals, post a short coordination update that includes: target file/branch, what you observed, and your next step.

### 3) Ask Before Editing Risky Files
Before editing a contested or uncertain file, run a check and follow it:

- Run `but-engineering check <file> --agent-id <your-id> --include-stack [--intent-branch <branch>]`.
- Treat `action_plan` as the source of truth for next steps.
- Execute required `action_plan` steps in ascending `priority` before editing.
- If a command includes placeholders (`<id>`, `<child>`, `<branch>`, `<message>`), fill only safe values or re-run `check`.
- If `decision` is `deny`, coordinate in channel and use a short wait/retry loop (do not idle for long):
  - `but-engineering read --agent-id <id> --wait --timeout 5s`
  - `but-engineering check <file> --agent-id <id> --include-stack [--intent-branch <branch>]`
  - Repeat until clear.
- If `decision` is `allow` with warnings, read context first, then proceed carefully.
- If `decision` is `allow` and `reason_code` is `no_conflict`, still run start `post` and one `read` before the first edit, unless `coordination_mode=exclusive_owner`.
- If `reason_code` is `stack_dependency`, post a dependency coordination message before continuing.
- If a `but commit` fails with dependency wording (for example "locked to commit", "assigned to another branch", or similar), treat it as stack dependency immediately:
  - run `but status --json`
  - run `but-engineering check <file> --agent-id <id> --include-stack [--intent-branch <branch>]`
  - post a short dependency coordination message
  - align with `but branch new <child> -a <base>` before retrying the commit
- If `coordination_mode` is `exclusive_owner`, proceed directly to the edit path (no extra read/post/check loop before the edit).
- For multi-file tasks, include all target files in one plan and run `check` per risky file before first edit of each file.
- If `reason_code=stack_dependency`, run the first required `action_plan` steps immediately (read/post/status/alignment) before dependent edits.
- If you are blocked and cannot make progress on a claimed file, release that file claim immediately:
  - `but-engineering release <file> --agent-id <id>`

## Execution Checklist (Order Matters)

Use this sequence for normal coordinated work:

1. `but-engineering plan --agent-id <id> "<plan>"`
2. `but-engineering post "<start message>" --agent-id <id>`
3. `but-engineering read --agent-id <id>`
4. `but-engineering check <file> --agent-id <id> --include-stack [--intent-branch <branch>]` before risky edits
5. Execute the first 1-2 required `action_plan` steps (and re-run `check` if `retry_check` appears)
   - If coordination is needed, post a brief update with file/branch + next action before editing.
   - If `coordination_mode=exclusive_owner`, do not run extra read/post/check loops before the first edit.
   - If blocked, use short cadence: `read --wait --timeout 5s` then `check` again.
   - If blocked and you cannot actively work that file now, release that file claim immediately.
6. Edit files
   - As soon as you finish a file (or switch away from it), release that file immediately:
     `but-engineering release <file> --agent-id <id>`
7. `but-engineering discover "<finding>" --agent-id <id>` for any gotcha/risk (mandatory when a `GOTCHA` marker is present)
8. `but-engineering done "<what you completed + key files>" --agent-id <id>`
9. Completion gate: verify the done announcement was posted before finishing.

Do not finish while your own claims remain, your plan is uncleared, or no completion announcement has been posted.

## Minimal Command Reference

- `but-engineering plan --agent-id <id> "<plan>"`
- `but-engineering plan --agent-id <id> --clear`
- `but-engineering post "<message>" --agent-id <id>`
- `but-engineering read --agent-id <id>`
- `but-engineering check <file> --agent-id <id> --include-stack [--intent-branch <branch>]`
- `but-engineering claim <files...> --agent-id <id>`
- `but-engineering release <files...> --agent-id <id>`
- `but-engineering release --all --agent-id <id>`
- `but-engineering discover "<finding>" --agent-id <id>`
- `but-engineering done "<completion summary>" --agent-id <id>`
- `but-engineering agents --active-within 5m`

## Operating Notes

- Keep coordination silent to the user unless it blocks delivery.
- Use one consistent, unique `--agent-id` for the session.
- Parse JSON outputs; do not print raw command output to users.
- Prefer `action_plan.commands` over ad-hoc coordination command phrasing for consistency.
- Keep the coordination loop lean for speed:
  - One `read` before first edit is mandatory; avoid additional redundant `read`, `agents`, or `status` calls unless blocked/uncertain.
  - If target files are already named in the task, skip `ls/find` discovery commands and run `check` directly.
  - Prefer one `claim` call for all files you will edit in this step.
  - While blocked, prefer short wait intervals (about 5-10s), then re-check quickly instead of long sleeps.
  - Release claims immediately when done with a file or when pausing work on that file due to contention.
  - Completion announcements should be specific: include what was done and the key file/area (e.g., `DONE: Added emojis to README.md and updated examples`).
