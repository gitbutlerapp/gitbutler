# `but link` Coordination Reference

You are a teammate in a shared repository. Other agents are working here too.
Coordination is broader than file locks: use it to align intent, share blockers/decisions, and provide clean handoffs.
You coordinate before editing, communicate clearly, and avoid stepping on active work.

## Hard Constraints (Avoid Tool Thrash)

- Use only `but link` for ALL coordination commands. Every coordination command must start with `but link`.
- Do NOT add legacy flags not supported by the link protocol (e.g. `--include-stack`).
- Do NOT run `which`, `--help`/`help`, `strings`, `netstat`, `strace`, `pwd`, `env`, or repo-wide greps.
- Assume `but link` works and is on PATH; if a coordination command exits `0`, treat it as successful even if it prints nothing.
- `but link read` reads coordination state (messages/claims/agents), not file contents. Use `sed -n '1,200p <file>'` or `cat <file>` to read source files.
- When a file is blocked/denied, check once, then **skip immediately**. Do NOT retry check/read loops on blocked files — move to the next file.

## Command Templates (copy exactly)

All commands support multiple `--path` flags for batch operations.

- `but link post "<message>" --agent-id <id>`
- `but link read --agent-id <id>`
- `but link check --path <file1> --path <file2> --format compact --agent-id <id>`
- `but link claim --path <file1> --path <file2> --ttl 15m --agent-id <id>`
- `but link release --path <file1> --path <file2> --agent-id <id>`
- `but link done "<summary>" --agent-id <id>`

## Execution Checklist (Order Matters)

Minimize coordination overhead — batch operations across files, don't coordinate per-file.

1. **Read state**: `but link read --agent-id <id>` — sync coordination state once at the start. If you see messages from other agents that relate to your work (analysis, triage, questions), reply before proceeding.
2. **Check all target files in one command** before editing any of them:
   `but link check --path <file1> --path <file2> --path <file3> --format compact --agent-id <id>`
   Output is one line per file: `allow src/foo.rs no_conflict` or `warn src/bar.rs claimed_by_other peer-a`.
   Sort files into two lists: **allowed** (can edit) and **blocked** (must skip).
3. **If any files are blocked**: post one message covering ALL blocked files — include each file path, its blocker/owner, and why you're skipping it. Use the word "skip" or "avoid" if a discovery message said to avoid a file.
4. **Claim all allowed files in one command**:
   `but link claim --path <file1> --path <file2> --ttl 15m --agent-id <id>`
5. **Edit all claimed files** — do the actual coding work. After editing, verify changes landed with `cat <file>` or `grep` for each edited file.
6. **Done**: `but link done "<summary>" --agent-id <id>` — mandatory, even if running low on turns. Done auto-releases all your claims, so no explicit release step is needed. The summary replaces per-file posts, so make it comprehensive.

## Read-Only Checklist (No File Edits)

Use this for PR review, triage, or analysis-only tasks where you are not editing files.

1. **Read state**: `but link read --agent-id <id>` — sync channel state first.
2. **Announce start**: `but link post "<what you are reviewing>" --agent-id <id>`.
3. **Run one meaningful pass**: for example, fetch new comments, inspect code paths, run validation commands.
4. **Post pass summary**: `but link post "<agree/disagree + key findings>" --agent-id <id>` after each meaningful pass.
5. **Repeat steps 3-4** for each additional review pass.

## Coordination Message Quality

Keep messages few but high-quality. **Always use full file paths** (e.g., `src/api.rs`, not just `api.rs` or `api`).

The channel is a team conversation, not just a lock protocol. Three message types:

- **Reply** (when relevant messages exist): if you read analysis, a triage, findings, or questions from another agent that relate to your work, reply with `@<agent-id>: ack:` plus your perspective. Don't leave teammates talking into the void. A short reply like `"@peer-a: ack: your triage matches what I landed — X, Y fixed; skipped Z (intentional)"` is enough.
- **Blocker post** (only if blocked files exist): list every blocked file with full path, its owner agent, and your action. If a discovery message said to avoid a file, echo "skip" or "avoid" — e.g., "Skipping src/api.rs — peer-c discovery says avoid, will skip."
- **Done summary** (always required): list every target file with full path and status. For each file, note the claim lifecycle:
  - `src/db.rs: completed, claimed fresh (no prior claim).`
  - `src/config.rs: completed, stale claim refreshed/renewed.`
  - `src/auth.rs: skipped, blocked by peer-a (active claim).`
  - `src/api.rs: skipped, peer-c discovery says avoid/skip.`

  Claim lifecycle keywords to include where applicable: "fresh claim", "expired" + "renew", "stale" + "refresh" or "renew", "claim". These help teammates understand the state of each file.
