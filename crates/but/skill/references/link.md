# `but link` Coordination Reference

You are a teammate in a shared repository. Other agents are working here too.
Coordination is broader than file locks: use it to align intent, share blockers/decisions, and provide clean handoffs.
You coordinate before editing, communicate clearly, and avoid stepping on active work.

## Hard Constraints (Avoid Tool Thrash)

- Use only `but link` for ALL coordination commands. Every coordination command must start with `but link`.
- Use one stable `agent-id` for the entire user session. Reuse it across reads, plans, acquires, reviews, and retries; do not generate a fresh id per pass or subtask.
- Do NOT add legacy flags not supported by the link protocol (e.g. `--include-stack`).
- Do NOT run `which`, `--help`/`help`, `strings`, `netstat`, `strace`, `pwd`, `env`, or repo-wide greps.
- Assume `but link` works and is on PATH; if a coordination command exits `0`, treat it as successful even if it prints nothing.
- `but link read` reads coordination state, not file contents. That includes free-text messages plus typed discoveries, blocks, claims, agents, and surfaces depending on the selected view. Use `sed -n '1,200p <file>'` or `cat <file>` to read source files.
- When a file is blocked/denied, do NOT spin in a tight loop. Set blocked status, wait with backoff, then retry acquire. If other unblocked work exists, do that while waiting.

## Command Templates (copy exactly)

All path-based commands support multiple `--path` flags for batch operations.

- `but link post "<message>" --agent-id <id>`
- `but link read --agent-id <id>`
- `but link read --view claims --agent-id <id>`
- `but link read --view agents --agent-id <id>`
- `but link plan "<what you're about to do>" --agent-id <id>`
- `but link status blocked --agent-id <id>`
- `but link acquire --path <file1> --path <file2> --ttl 15m --agent-id <id>`
- `but link acquire --path <file1> --path <file2> --dry-run --format compact --agent-id <id>`
- `but link block --path <file1> --reason "<reason>" --mode advisory --agent-id <id>`
- `but link ack --agent <other-agent> --path <file1> --note "<note>" --agent-id <id>`
- `but link resolve --block-id <id> --agent-id <id>`
- `but link done "<summary>" --agent-id <id>`

## Execution Checklist (Order Matters)

Minimize coordination overhead — batch operations across files, don't coordinate per-file.

1. **Read state**: `but link read --agent-id <id>` — this returns your inbox by default. Read directed updates and your own pending acknowledgements first. Path-scoped typed blocks and advisories become relevant once you have active claims or switch to a more specific read view.
   `read --view messages` is free-text only. `read --view claims` and `read --view agents` are the canonical observer views for ownership/status. `read --view full` is a structured snapshot with separate `messages`, `discoveries`, `claims`, `agents`, `blocks`, and `surfaces` sections rather than a single transcript containing typed coordination events.
2. **Announce intended work** before acquiring anything:
   `but link plan "<what you're about to do>" --agent-id <id>`
   This is what makes your intended work show up in the TUI immediately, before claims or edits land.
3. **Acquire all target files in one command** before editing any of them:
   `but link acquire --path <file1> --path <file2> --path <file3> --ttl 15m --agent-id <id>`
   Output is one decision per file. `acquired` means the file is now safely claimed. `blocked` means do not edit it.
   Use `--dry-run` when you want the same decision payload without taking the claim. When a decision is blocked/warn/deny, read `retry_after_ms` / `retry_at_ms` when present and use them as your next retry hint instead of abandoning the work immediately.
4. **If any files are blocked**: do not give up on the first denial. Coordinate and retry with backoff:
   - set `but link status blocked --agent-id <id>` so the TUI shows that you are waiting
   - wait `retry_after_ms` from the acquire decision when present, otherwise wait 30s
   - run `but link read --agent-id <id>` again, then reacquire the blocked paths
   - if other unblocked work exists, do that while waiting
   - use `but link ack` to acknowledge another agent's typed update
   - use `but link block` if you need to publish your own advisory/hard blocker
   - use `but link resolve` only to close a specific typed block
5. **Edit only acquired files** — do the actual coding work. After editing, verify changes landed with `cat <file>` or `grep` for each edited file.
6. **Done**: `but link done "<summary>" --agent-id <id>` — mandatory, even if running low on turns. Done auto-releases all your claims.

## Read-Only Checklist (No File Edits)

Use this for PR review, triage, or analysis-only tasks where you are not editing files.

1. **Read state**: `but link read --agent-id <id>` — sync your inbox first.
2. **Announce start**: `but link post "<what you are reviewing>" --agent-id <id>`.
3. **Run one meaningful pass**: for example, fetch new comments, inspect code paths, run validation commands.
4. **Post pass summary**: `but link post "<agree/disagree + key findings>" --agent-id <id>` after each meaningful pass.
5. **Repeat steps 3-4** for each additional review pass.

## Coordination Message Quality

Keep messages few but high-quality. **Always use full file paths** (e.g., `src/api.rs`, not just `api.rs` or `api`).

The channel is a team conversation, but typed state is authoritative for blockers/acks/resolution. Free-text posts are for visibility and handoff quality, not for creating coordination state.

- **Reply** (when relevant messages exist): if you read analysis, a triage, findings, or questions from another agent that relate to your work, record authoritative acknowledgement with `but link ack --agent <peer> --note "<summary>" --agent-id <id>`. Add a free-text `but link post` only when you need extra commentary or handoff detail.
- **Typed coordination**: use `but link block`, `but link ack`, and `but link resolve` to create authoritative coordination state. Do not rely on free-text words like "blocked", "skip", or "ack" to drive decisions.
- **Discovery formatting**: use `but link read --view discoveries --format full|brief|digest --agent-id <id>` instead of the removed `brief` / `digest` subcommands.
- **Done summary** (always required): list every target file with full path and status. For each file, note the claim lifecycle:
  - `src/db.rs: completed, claimed fresh (no prior claim).`
  - `src/config.rs: completed, stale claim refreshed/renewed.`
  - `src/auth.rs: skipped, blocked by peer-a (active claim).`
  - `src/api.rs: skipped, peer-c discovery says avoid/skip.`

  Claim lifecycle keywords to include where applicable: "fresh claim", "expired" + "renew", "stale" + "refresh" or "renew", "claim". These help teammates understand the state of each file.
