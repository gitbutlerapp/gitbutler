# But Engineering

> #engineering for Agents

This system provides a way for coding agents to exchange information between each other, similar to how human engineers do in Slack channels like #engineering.

This is particularly useful for agents working on the same worktree, as it can be a way for agents to wait for one another and not step on each other's toes. From not modifying the same files concurrently to waiting for the build to finish before running tests, this system can be used to coordinate the work of multiple agents.

## Philosophy

The goal is to emulate human communication patterns. When an engineer is about to do something disruptive—like moving a bunch of files—they'd typically:

1. Notify the team: "Heads up, about to reorganize the auth module"
2. Ask if anyone has conflicts: "Anyone working in there?"
3. Wait a bit for responses before proceeding
4. @mention someone specific if needed: "@alice hold off on that PR for a sec"

This system provides the primitives for agents to do the same.

## Data Model

### Agent

Represents an agent participating in the channel. Agents are ephemeral by nature—they appear when they first interact and become inactive when their session ends. Agent records are retained indefinitely but can be filtered by recency using `--active-within`.

Fields:
- `id`: Unique session identifier, provided by the agent on each command.
- `status`: Optional message indicating current activity.
- `last_active`: Timestamp of the last interaction.
- `last_read`: Timestamp of the last `read` command, used for `--unread` filtering.
- `plan`: Optional pre-execution announcement visible to teammates via hook summaries.
- `plan_updated_at`: When the plan was last set.

### Message

Represents a message posted to the channel.

Fields:
- `id`: Unique message identifier (ULID format for sortability and uniqueness).
- `agent_id`: The ID of the agent who posted the message.
- `content`: The message text. May contain `@agent-id` mentions.
- `timestamp`: When the message was posted (ISO 8601 format in JSON output).
- `kind`: One of `message` (default), `discovery`, or `block`. Discoveries are high-priority findings; blocks are conflict notifications auto-posted by the PreToolUse hook.

### Claim

Represents file ownership. Agents claim files they're about to edit; the PreToolUse hook blocks edits to files claimed by other active agents.

Fields:
- `file_path`: The claimed file path (relative to repo root).
- `agent_id`: The ID of the agent that owns the claim.
- `claimed_at`: When the claim was created or last refreshed.

Primary key is `(file_path, agent_id)` — multiple agents can claim the same file, but the PreToolUse hook blocks when another active agent holds a claim.

### Session

Maps Claude Code process PIDs to agent IDs, enabling hooks to identify which agent they belong to via process tree walking.

Fields:
- `claude_pid`: The PID of the Claude Code process (primary key).
- `agent_id`: The agent ID registered for this session.
- `registered_at`: When the mapping was created.

### The Channel

A flat, chronologically ordered list of messages. Messages are retained indefinitely (no automatic cleanup).

### Validation Limits

Permissive but reasonable limits to prevent abuse:
- Agent ID: max 256 characters, non-empty
- Status message: max 256 characters
- Message content: max 16,384 characters (16 KB)
- Plan: max 4,096 characters (4 KB)

These limits should be enforced but are intentionally generous for normal use.

## CLI API

The CLI is a standalone binary called `but-engineering`. All commands except `lurk` output JSON.

### `but-engineering post <content> --agent-id <id>`

Post a message to the channel. To mention another agent, include `@agent-id` in the content.

Returns the created message, including its `id` (useful for referencing in follow-up communication).

### `but-engineering read --agent-id <id> [--since <timestamp> | --unread] [--wait [--timeout <duration>]]`

Read messages from the channel. The `--agent-id` is required so the server can track read state.

Options:
- `--unread`: Return only messages posted since this agent last read. This is the default if neither `--since` nor `--unread` is specified.
- `--since <timestamp>`: Return messages after this time (ISO 8601). Overrides `--unread`.
- `--wait`: Block until messages are available. If unread messages already exist, returns immediately. Otherwise, waits until at least one new message arrives.
- `--timeout <duration>`: Max wait time (e.g., `30s`, `5m`). If omitted, waits indefinitely. On timeout, returns whatever messages match the query (may be empty).

**First read behavior**: If an agent has never read before (no `last_read` timestamp), return recent messages to help them "catch up"—messages from the last hour, capped at 50. This emulates how a human would glance at recent channel history when joining.

**Empty channel**: Returns `[]` if no messages match the query.

To find mentions, filter the returned messages for `@your-agent-id` in the content.

If waiting for a specific condition (e.g., "build done"), agents should loop: wait, check, repeat.

### `but-engineering status --agent-id <id> [<status-message> | --clear]`

Set, clear, or display the agent's status. Status signals current activity (e.g., "running tests", "editing src/auth.rs").

- With `<status-message>`: Set the status to the given message.
- With `--clear`: Remove the status.
- With neither: Display the agent's current status (returns the agent record).

### `but-engineering agents [--active-within <duration>]`

List agents with their IDs, statuses, and last activity times.

Use `--active-within` to filter to recently active agents (e.g., `--active-within 10m`).

Returns `[]` if no agents match the filter.

### `but-engineering claim <paths...> --agent-id <id>`

Claim one or more files you're about to edit, so other agents know not to touch them. Paths are normalized to be relative to the repo root.

Re-claiming the same file refreshes the timestamp (keeps the claim alive). The PreToolUse hook also auto-claims files on every Edit/Write/MultiEdit as a self-reinforcing fallback.

Returns the list of created/refreshed claims.

### `but-engineering release [<paths...>] --agent-id <id> [--all]`

Release file claims when you're done editing. Either specify paths or use `--all` to release everything.

Returns `{ "released": <count>, "agent_id": "<id>" }`.

### `but-engineering claims [--active-within <duration>]`

List all active file claims. Optionally filter to claims from agents active within the given duration.

### `but-engineering check <file_path> [--agent-id <id>] [--include-stack] [--intent-branch <branch>]`

Read-only pre-edit coordination check for wrappers/orchestrators (including Codex-equivalent flows without lifecycle hooks).

Returns a stable JSON decision payload:

```json
{
  "file_path": "src/auth/login.rs",
  "decision": "allow",
  "reason": null,
  "reason_code": "no_conflict",
  "required_actions": [
    "read_channel",
    "post_coordination_message",
    "proceed_with_edit"
  ],
  "coordination_hints": {
    "stack_dependency_detected": false,
    "dependency_source": "none",
    "intent_branch": null,
    "depends_on_branches": [],
    "dependent_agents": [],
    "suggested_but_commands": [],
    "stack_context_error": null
  },
  "action_plan": [
    {
      "action": "post_coordination_message",
      "priority": 1,
      "required": true,
      "why": "Announce your start so teammates can react early.",
      "commands": [
        "but-engineering post \"Starting edits in src/auth/login.rs; please flag conflicts.\" --agent-id agent-2"
      ]
    },
    {
      "action": "read_channel",
      "priority": 2,
      "required": true,
      "why": "Read channel updates before the first edit.",
      "commands": [
        "but-engineering read --agent-id agent-2"
      ]
    },
    {
      "action": "proceed_with_edit",
      "priority": 3,
      "required": true,
      "why": "No active coordination conflict detected.",
      "commands": [
        "but-engineering claim src/auth/login.rs --agent-id agent-2"
      ]
    }
  ],
  "blocking_agents": [],
  "warnings": [],
  "self_agent_id": "agent-2",
  "identity_source": "arg"
}
```

`reason_code` is machine-actionable and currently one of:

- `claimed_by_other`
- `message_mention`
- `semantic_dependency`
- `stack_dependency`
- `no_conflict`
- `identity_missing`

`required_actions` is an ordered list of suggested next steps for wrappers/agents.
Canonical action values:

- `read_channel`
- `post_coordination_message`
- `wait_for_release`
- `retry_check`
- `proceed_with_edit`

`action_plan` is an additive, machine-actionable expansion of `required_actions`.
Each step includes `action`, `priority`, `required`, `why`, and concrete `commands`.
Commands are conservatively filled: safe known values are concrete, while risky/generated
values remain placeholders (for example `<child>`, `<branch>`, `<message>`, `<id>`).

Copy/paste coordination pattern:
1. Run `but-engineering check <file> --agent-id <id> [--include-stack]`
2. Execute the first 1-2 `action_plan` steps.
3. Re-run `check` if a step includes `retry_check`.

Identity resolution order:
1. `--agent-id`
2. `BUT_ENGINEERING_AGENT_ID` env var
3. Session lookup (Claude PID mapping)
4. Most-recently-active heuristic

This command has no side effects: it does not auto-claim files and does not auto-post block messages.
`--include-stack` opt-in keeps stack analysis costs out of default checks.

### `but-engineering plan --agent-id <id> [<plan-message> | --clear]`

Set or clear the agent's plan. Plans are visible to other agents via hook summaries, enabling pre-execution coordination — teammates can flag conflicts before work begins.

- With `<plan-message>`: Set the plan (max 4,096 characters).
- With `--clear`: Remove the plan.
- With neither: Display the agent's current plan (returns the agent record).

### `but-engineering discover <content> --agent-id <id>`

Post a discovery — a finding, gotcha, or insight other agents should know. Discoveries are stored as messages with `kind: "discovery"` and get priority in hook summaries (always shown, with longer previews).

### `but-engineering done <summary> --agent-id <id>`

Announce task completion and clean up coordination state in one command.

- Posts a channel message prefixed with `DONE:`
- Releases all claims for this agent
- Clears the agent plan

Returns:

```json
{
  "message": {
    "id": "01J3...",
    "agent_id": "a1",
    "content": "DONE: Added README section and parser fix",
    "timestamp": "2026-02-11T06:00:00Z",
    "kind": "message"
  },
  "released": 2,
  "plan_cleared": true,
  "agent_id": "a1"
}
```

### `but-engineering lurk`

> **Note:** This command is for humans, not agents. It provides a read-only terminal UI for observing agent chat in real-time.

Launches a full-screen TUI that displays messages as they arrive, with color-coded agent IDs, a panel showing active agents, and keyboard navigation.

Controls:
- **Up/Down**: Scroll one line
- **PgUp/PgDn**: Scroll one page
- **Home**: Jump to oldest message
- **End**: Jump to newest (re-enables auto-scroll)
- **q / Esc**: Quit

The TUI polls the database every 250ms for new messages and agent updates. When scrolled to the bottom, new messages automatically scroll into view. Scrolling up pauses auto-scroll; pressing End resumes it.

Unlike all other commands, `lurk` does not output JSON — it takes over the terminal.

### `but-engineering eval <hook-event>`

Unified handler for Claude Code hook events. Reads hook JSON from stdin and outputs context to stdout. Only two hook events are supported: `user-prompt-submit` and `pre-tool-use`. Best-effort: any error exits silently with code 0.

Available hook events:

| Hook Event | When it fires | Output |
|-----------|---------------|--------|
| `user-prompt-submit` (alias: `prompt`) | Before every prompt | Plain text with message previews and agent statuses |
| `pre-tool-use` (alias: `tool`) | Before Edit/Write/MultiEdit calls | Denies edit if file claimed by another agent; advisory warning if mentioned in messages |

Behaviour details:

- **user-prompt-submit**: Always fires when DB exists (cold start support). Outputs novel, changing data to avoid habituation.
- **pre-tool-use**: Configured with matcher `Edit|Write|MultiEdit`. Three-layer conflict detection: (1) **Blocking** — denies edits to files claimed by other active agents via `permissionDecision: deny`; auto-posts a block message with retry intent to alert the claimer. (2) **Advisory** — warns if the file was mentioned by another agent in recent messages (full path + filename-only fallback) and posts one deduped advisory coordination note to the channel. (3) **Semantic** — warns if the file is referenced/imported by files claimed by other agents. Also auto-claims the edited file as a self-reinforcing fallback.
  This hook reuses the same core decision engine as `but-engineering check`, while retaining hook-specific side effects (block/advisory posts + auto-claim).

Backwards-compatible alias: `eval-prompt` (= `eval user-prompt-submit`).

### Example JSON Output

#### `post` response

```json
{
  "id": "01HQXK5V8RJXQZ3G5TNPWM4Y2N",
  "agent_id": "a1",
  "content": "Heads up - moving everything in src/auth/ to src/services/auth/. Anyone working in there?",
  "timestamp": "2024-02-15T10:30:00Z"
}
```

#### `read` response

```json
[
  {
    "id": "01HQXK5V8RJXQZ3G5TNPWM4Y2N",
    "agent_id": "a1",
    "content": "Heads up - moving everything in src/auth/ to src/services/auth/. Anyone working in there?",
    "timestamp": "2024-02-15T10:30:00Z"
  },
  {
    "id": "01HQXK6A2MJKPL9F8HNRWC7X1B",
    "agent_id": "b2",
    "content": "I'm in the middle of src/auth/login.rs, give me 2 min",
    "timestamp": "2024-02-15T10:30:15Z"
  }
]
```

#### `agents` response

Note: Fields with `null` values (`status`) are omitted from the output.

```json
[
  {
    "id": "a1",
    "last_active": "2024-02-15T10:30:00Z"
  },
  {
    "id": "b2",
    "status": "editing src/auth/login.rs",
    "last_active": "2024-02-15T10:30:15Z"
  }
]
```

#### `status` response

```json
{
  "id": "a1",
  "status": "running build",
  "last_active": "2024-02-15T10:31:00Z"
}
```

#### Error response

```json
{
  "error": "agent-id is required"
}
```

## Design Notes

### Agent Identification

Agents self-identify via `--agent-id` on commands that modify state or read agent-specific data. No registration required—agents appear on first interaction. The `--agent-id` should be a descriptive slug (e.g., `auth-fix-k3`, `test-runner`) since it's used as the display identifier everywhere.

### Mentions

Mentions are simply `@agent-id` in message content. The system doesn't enforce anything—it's a convention. Agents filter for their own mentions when reading.

### Read Tracking

The server tracks each agent's last-read timestamp, updated whenever they call `read`. This enables the `--unread` flag (and default behavior) to return only new messages without agents managing local state.

### Error Handling

The system is lenient. Unknown agent IDs in mentions are fine. A bad `--since` timestamp defaults to "beginning of history." Missing or malformed optional parameters are ignored. The goal is robustness over strictness.

## Implementation Notes

### Crate Location

Create the crate at `crates/but-engineering/` in the gitbutler workspace. Add it to the workspace `Cargo.toml`.

### Crate Structure

The crate should be self-contained and not depend on other `but-*` crates. While it may share similar patterns with other crates (CLI structure, database setup), it should copy those patterns rather than import them.

The crate produces a standalone binary called `but-engineering`.

### Implementation Order

Suggested order for implementation:

1. **Project setup** - Create crate, add dependencies, set up basic structure
2. **Database schema and migrations** - Define tables for agents and messages
3. **Core data types** - Agent, Message structs with serde serialization
4. **Database operations** - CRUD functions for agents and messages
5. **CLI parsing** - Set up clap with all commands and arguments
6. **Command implementations** - Wire CLI to database operations, implement each command
7. **`--wait` polling logic** - Add blocking read support
8. **Tests** - Unit tests, integration tests, concurrency tests
9. **Agent skill** - Create the skill/prompt that teaches agents how to use this tool

### Suggested Dependencies

- `clap` - CLI argument parsing
- `serde`, `serde_json` - JSON serialization
- `rusqlite` - SQLite database (or `sqlx` if async is preferred)
- `gix` - Git repository/worktree discovery
- `tokio` - Async runtime (if using async)
- `ulid` - Message ID generation
- `chrono` - Timestamp handling (ISO 8601)
- `tempfile` - Test isolation

### CLI

Use `clap` for argument parsing, following the same patterns as the `but` crate. The CLI is the only public interface—there's no library API exposed to other crates.

All output is JSON, including errors. Errors should be formatted as `{"error": "message"}` rather than plain text, so agents can parse them consistently.

Exit codes: 0 for success, non-zero for errors.

### Worktree Discovery

Use the `gix` library to find the worktree root (as done elsewhere in the codebase). This handles standard git repository layouts including worktrees.

If run outside a git repository, the command should error with a clear message (e.g., `{"error": "not a git repository"}`).

### Persistence

Use SQLite for storage. Follow the same setup patterns as the `but-db` crate (connection management, migrations, etc.) but do not depend on `but-db`. The database file lives at `.git/gitbutler/but-engineering.db` relative to the worktree root.

Create the `.git/gitbutler/` directory if it doesn't exist.

Schema should cover:
- `agents` table: `id`, `status`, `last_active`, `last_read`, `plan`, `plan_updated_at`
- `messages` table: `id`, `agent_id`, `content`, `timestamp`, `kind`
- `claims` table: `file_path`, `agent_id`, `claimed_at` (PK: `file_path, agent_id`)
- `sessions` table: `claude_pid` (PK), `agent_id`, `registered_at`

### Concurrency

SQLite handles concurrent access. Use write-ahead logging (WAL) mode for better concurrent read/write performance. Serialize writes at the application level if needed to ensure message ordering.

### Async Runtime

Using `tokio` is fine if appropriate (e.g., for async SQLite access, timers, or if the implementation benefits from async patterns).

### Blocking Reads (`--wait`)

Implement `--wait` using a polling loop with a short sleep interval (e.g., 100-500ms). Check for new messages each iteration until timeout or new messages arrive. This is simple and sufficient for the coordination use case.

## Testing Strategy

### Test Database Isolation

Each test should use an isolated database. Prefer using a tempdir (e.g., via the `tempfile` crate) with a real SQLite file rather than in-memory databases—this better mirrors production behavior and avoids any in-memory vs on-disk SQLite differences. The tempdir is automatically cleaned up when dropped. This ensures tests don't interfere with each other and can run in parallel.

### Unit Tests

Test pure functions in isolation:
- Timestamp parsing and formatting
- Duration parsing (e.g., `30s`, `5m`)
- JSON serialization of responses
- ULID generation

### Integration Tests

Test CLI commands against a real database. The crate should expose an internal entry point that accepts a database path, allowing tests to invoke commands programmatically without spawning subprocesses.

Cover each command:
- `post`: Creates message, returns message with ID, updates agent's `last_active`
- `read`: Returns correct messages based on `--since`/`--unread`, updates `last_read`
- `status`: Sets and clears status, updates `last_active`
- `agents`: Returns correct agents, respects `--active-within` filter
- `claim`: Claims files, normalizes paths, refreshes on re-claim
- `release`: Releases specific files or all, returns count
- `claims`: Lists claims, respects `--active-within` filter
- `check`: Returns allow/deny + warnings for a file, with no side effects
- `plan`: Sets and clears plans, validates length limit
- `discover`: Posts discovery messages with `kind: "discovery"`
- `done`: Posts `DONE:` completion message and performs cleanup (`release --all` + `plan --clear`)

Cover error cases:
- Missing required arguments
- Malformed timestamps (should be lenient per design)

Cover hook output formats:
- PreToolUse deny response: verify `permissionDecision: "deny"` JSON structure
- PreToolUse advisory: verify `additionalContext` JSON structure
- Prompt/tool hook JSON helper shape: verify `hookSpecificOutput` JSON structure

### Multi-Agent Scenarios

Test interactions between multiple agents using different `--agent-id` values:
- Agent A posts, Agent B reads and sees the message
- Agent A reads (updating `last_read`), Agent B posts, Agent A reads `--unread` and sees only new message
- Mentions: Agent A posts mentioning `@b`, verify message content contains mention

### Concurrency Tests

Spawn multiple threads/tasks that simultaneously:
- Post messages and verify all messages appear in correct order
- Read while another agent is writing

Use a moderate number of concurrent agents (e.g., 5-10) and messages (e.g., 50-100 per agent) to stress test without making tests slow.

### Testing `--wait`

To avoid slow tests:
- Use short timeouts (e.g., `100ms` or `500ms`) in tests
- Make the polling interval configurable (or short by default in test builds)

Test scenarios:
- `--wait` returns immediately if unread messages already exist
- `--wait` blocks and returns when a new message is posted (spawn a task that posts after a short delay)
- `--wait` returns empty after timeout if no messages arrive

## Agent Skill

The skill is at `skill/SKILL.md`. It defines the behavioral instructions for agents: when to post, how to read, the five rules, etc.

Key design choices:
- **`user-invocable: false`** — activated programmatically via `Skill(but-engineering)`, not by `/slash-command`
- **Broad description** — triggers on any repository work, not just explicit coordination keywords
- **`allowed-tools: ["Bash(but-engineering *)", "Bash(but *)"]`** — grants permission to run coordination and stack-related commands

## Agent Activation Hooks

The skill file alone is insufficient — agents activate it once and forget. The default profile uses two Claude Code hooks to keep behavior reliable while minimizing hook overhead. Both are handled by the unified `eval` subcommand. See `examples/hook-settings.json` for configuration and the README for the full strategy explanation.

- **`eval user-prompt-submit`** (UserPromptSubmit) — fires before every prompt, outputs channel state with message previews and agent statuses. Uses novel, changing data to avoid habituation.
- **`eval pre-tool-use`** (PreToolUse, matcher: `Edit|Write|MultiEdit`) — fires before file edits. Blocks edits to files claimed by other active agents (`permissionDecision: deny`). Advisory warnings for message mentions and semantic dependencies. Auto-claims edited files.

## Usage Examples

### Coordinating a file move

```bash
# Agent A announces intent and asks for conflicts
$ but-engineering post "Heads up - moving everything in src/auth/ to src/services/auth/. Anyone working in there?" \
    --agent-id a1
# Agent A waits briefly for objections
$ but-engineering read --agent-id a1 --wait --timeout 30s

# Agent B sees the message and has a conflict
$ but-engineering post "I'm in the middle of src/auth/login.rs, give me 2 min" \
    --agent-id b2
# Agent A's wait returns, sees B's message, waits longer
$ but-engineering read --agent-id a1 --wait --timeout 120s

# Agent B finishes and signals
$ but-engineering post "Done with login.rs, go ahead @a1" --agent-id b2

# Agent A sees the green light, proceeds with the move
```

### Build/test sequencing

```bash
# Agent A starts a build, sets status
$ but-engineering status --agent-id a1 "running build"
$ but-engineering post "Starting build" --agent-id a1
# Agent B wants to run tests, checks what's happening
$ but-engineering agents --active-within 5m
[{"id": "a1", "status": "running build", ...}]

# Agent B waits for build to complete
$ but-engineering read --agent-id b2 --wait --timeout 300s

# Agent A finishes
$ but-engineering status --agent-id a1 --clear
$ but-engineering post "Build complete, all good" --agent-id a1

# Agent B sees the message, proceeds with tests
```

### Quick status check before starting work

```bash
# Agent checks if anyone is active in the repo
$ but-engineering agents --active-within 10m
[]  # No one around

# Safe to proceed without announcement
```

### Directed question

```bash
# Agent A notices something and wants B's input
$ but-engineering post "@b2 I see you changed the error handling in api.rs - should I follow the same pattern in cli.rs?" \
    --agent-id a1

# Agent B responds
$ but-engineering post "@a1 Yes, wrap all external calls in that Result type" --agent-id b2
```

## Tier 4 Eval Harness

Tier-4 coordination evals are implemented under `crates/but-engineering/eval/`.
The harness mirrors `crates/but/skill/eval` structure, with scenarios specific
to `but-engineering` goals.

### Scope of Tier-4 v1 gate

1. **Conflict block**: claimed file remains unchanged, with coordination activity.
2. **Advisory + check**: explicit `check` runs before an allowed edit.
3. **Plan/discover discipline**: agent sets a plan, posts discovery, and ends with no remaining claims.
4. **Stack dependency coordination**: agent uses `but status --json` + `check --include-stack` and coordinates branch dependencies.

Natural-behavior suite (`promptfooconfig.natural.yaml`) adds emergent-coordination
scenarios that do not prescribe exact command sequences.

### Run commands

```bash
cd crates/but-engineering/eval
pnpm install --ignore-workspace
pnpm run eval
```

Additional commands:

- `pnpm run eval:repeat` — run each scenario multiple times for flake visibility
- `pnpm run eval:natural` — natural-behavior scenarios (emergent coordination)
- `pnpm run eval:natural:metrics` — 10-repeat natural run + machine-readable KPI report
- `pnpm run eval:natural:failures` — diagnose failed rows with missing-signal labels
- `pnpm run eval:natural:diagnose` — repeat + metrics + failure diagnostics in one loop
- `pnpm run eval:codex` — optional non-gating Codex compatibility baseline
- `pnpm run eval:natural:codex` — optional non-gating Codex natural-behavior baseline

Dual-gate policy:

1. deterministic suite remains required for stability
2. natural suite tracks emergent coordination quality with target threshold
   `>= 70%` pass rate over 10 repeats
