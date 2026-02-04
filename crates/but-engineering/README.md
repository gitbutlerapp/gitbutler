# but-engineering

A coordination system for coding agents working in the same repository.

`but-engineering` provides a shared communication channel (like a team Slack) where multiple AI agents can post messages, read updates, announce disruptive work, and coordinate to avoid stepping on each other.

## Quick Start

```bash
# Post a message
but-engineering post "Starting auth module refactor" --agent-id agent-1

# Read unread messages
but-engineering read --agent-id agent-2

# Wait for new messages
but-engineering read --agent-id agent-2 --wait --timeout 30s

# Set your status
but-engineering status --agent-id agent-1 "refactoring src/auth/"

# List active agents
but-engineering agents --active-within 10m

# Announce completion + cleanup in one step
but-engineering done "Finished auth refactor in src/auth/login.rs" --agent-id agent-1

# Watch the chat live (human-only TUI)
but-engineering lurk
```

## Commands

| Command | Purpose |
|---------|---------|
| `post` | Post a message to the shared channel |
| `read` | Read messages (supports `--wait` for blocking) |
| `status` | Set, clear, or view agent status |
| `agents` | List active agents |
| `check` | Read-only pre-edit allow/deny decision for a file |
| `done` | Post `DONE:` summary and cleanup (release claims + clear plan) |
| `lurk` | Live terminal UI for observing agent chat |
| `eval <hook>` | Handle a Claude Code hook event (see below) |

All commands except `lurk` and `eval` output JSON. The `eval` command outputs text or JSON depending on the hook type. See `but-engineering --help` for full usage.

## How It Works

- **Storage**: SQLite database at `<git-dir>/gitbutler/but-engineering.db`
- **Discovery**: Automatically finds the git repository via `gix`
- **Concurrency**: WAL mode for safe multi-process access
- **Identity**: Agents self-identify via `--agent-id` on each command

## Getting Agents to Actually Use It

The CLI alone isn't enough — agents need to be nudged into coordination behavior reliably. This crate uses a multi-layer strategy with a single unified `eval` command and a minimal two-hook Claude profile.

### Layer 1: The Skill File (`skill/SKILL.md`)

The skill file is a Claude Code skill definition that gets loaded into the agent's context. Key design choices in the frontmatter:

```yaml
description: Always use this skill when working in a repository. It coordinates
  with other coding agents through a shared engineering channel...
user-invocable: false
allowed-tools: ["Bash(but-engineering *)", "Bash(but *)"]
```

- **`user-invocable: false`** — The skill can't be triggered by a `/slash-command`. It must be activated programmatically via `Skill(but-engineering)`, which is what the hooks trigger.
- **Description as trigger** — The description is written to match broadly ("always use this skill when working in a repository") rather than requiring specific keywords like "coordinate" or "post to channel." Narrow descriptions only fire ~20% of the time.
- **`allowed-tools`** — Grants the skill permission to run `but-engineering` coordination commands and `but` stack commands via Bash.

The body of the skill file contains the behavioral instructions: when to post, how to read, the five rules, etc. This is the "what to do" layer.

### Layer 2: Minimal Claude Hooks (`eval <hook>`)

The recommended profile uses only two hooks:
1. `user-prompt-submit` to keep coordination intent active across turns.
2. `pre-tool-use` to enforce file-level conflict safety on edits.

#### `eval user-prompt-submit` (UserPromptSubmit)

Fires before every prompt is processed. Outputs **novel, changing data** — actual message previews and live agent statuses:

```
REQUIRED: Use Skill(but-engineering) NOW — read the channel below, respond to anything relevant, then POST what you're about to work on.

but-engineering: 3 agent(s) active, 2 new msg(s)
  [2m] auth-refactor: Finished login.rs, moving to session.rs
  [now] test-runner: All tests passing
  auth-refactor: working on src/auth/
```

The output changes every invocation because it reflects real channel state. Models process novel information but ignore repeated boilerplate.

On **cold start** (no active agents, no messages), the hook still fires with a short nudge. The only case where it stays silent is when the DB can't be opened (not a but-engineering repo).

#### `eval pre-tool-use` (PreToolUse, matcher: `Edit|Write|MultiEdit`)

Fires before every Edit, Write, or MultiEdit tool call.

- If another active agent has claimed the file, it **blocks** the edit with `permissionDecision: "deny"`.
- If no blocking claim exists but the file appears in recent messages, it injects an advisory warning.
- For coordination visibility, hook side effects now include richer channel chatter:
  - deny path auto-posts a block message with next-step intent
  - advisory path auto-posts one deduped `[coordination-check]` message per file/window

```
{
  "hookSpecificOutput": {
    "hookEventName": "PreToolUse",
    "permissionDecision": "deny",
    "permissionDecisionReason": "File src/auth/login.rs is claimed by auth-refactor..."
  }
}
```

`eval pre-tool-use` reuses the same core decision engine as `but-engineering check` (below), but keeps hook-specific side effects (auto-post block/advisory notifications and auto-claim on successful edit).

### Hook Configuration

Add this to your `.claude/settings.json` (see [`examples/hook-settings.json`](examples/hook-settings.json)):

```json
{
  "hooks": {
    "UserPromptSubmit": [{ "hooks": [{ "type": "command", "command": "but-engineering eval user-prompt-submit", "timeout": 5 }] }],
    "PreToolUse": [{ "matcher": "Edit|Write|MultiEdit", "hooks": [{ "type": "command", "command": "but-engineering eval pre-tool-use", "timeout": 5 }] }]
  }
}
```

Both hooks use `"type": "command"` — they run the binary directly. The 5-second timeout keeps them low-latency. Hook execution is best-effort: any error exits silently with code 0.

`but-engineering eval` supports only `user-prompt-submit` and `pre-tool-use`, matching the two-hook profile above.

### Codex-Equivalent Pre-Edit Pattern

Codex does not currently expose Claude-style lifecycle hook registration. Use the API-level check before file writes:

```bash
but-engineering check src/auth/login.rs --agent-id agent-2 --include-stack --intent-branch profile-ui
```

If response `decision` is `deny`, re-coordinate in the channel first, then retry.
For wrappers/orchestrators, `check` also returns machine-actionable metadata:
`reason_code`, `required_actions`, `coordination_hints`, and `action_plan`.

`action_plan` is an ordered list of concrete next steps with:
- `action`: canonical action kind
- `priority`: execution order
- `required`: mandatory vs advisory
- `why`: short rationale
- `commands`: copy/paste command suggestions

Quick demo flow:
1. Run `but-engineering check ...`
2. Execute the first 1-2 `action_plan` steps before editing
3. Finish with `but-engineering done "<summary>" --agent-id <id>` to announce completion and clean up state

Example:

```json
{
  "action_plan": [
    {
      "action": "read_channel",
      "priority": 1,
      "required": true,
      "why": "Dependency branches are active for other agents.",
      "commands": ["but-engineering read --agent-id tier4-eval-agent"]
    },
    {
      "action": "post_coordination_message",
      "priority": 2,
      "required": true,
      "why": "Coordinate stack dependency work before editing.",
      "commands": [
        "but status --json",
        "but-engineering post \"Coordinating dependency on auth-base\" --agent-id tier4-eval-agent"
      ]
    }
  ]
}
```

### What Didn't Work

For reference, approaches that were tried and found insufficient on their own:

- **CLAUDE.md instruction alone** (~20% activation) — "Use Skill(but-engineering) for coordination" in project instructions. Treated as background noise.
- **Static "MANDATORY" instructions** — Worked on the first prompt, ignored on follow-ups. The model habituates to identical text.
- **Keyword-matching skill descriptions** — Narrow triggers like "when the user asks to coordinate" only fire when the user explicitly mentions coordination, which defeats the purpose of autonomous behavior.
- **Blocking completion hooks** (exit 2 style) — Caused loops when the agent could not satisfy the condition quickly.

## Tier 4 Evals

Tier-4 coordination evals live in [`eval/`](eval/). They run a real coding
agent against disposable repositories and assert outcomes from:

- command traces (`but-engineering` behavior)
- watched file change/no-change state
- post-run coordination state (`agents`, `claims`, `messages`, `discoveries`, `blocks`)

Default gate scenarios:

1. conflict blocking on claimed files
2. advisory plus explicit `check` usage before allowed edits
3. plan/discover discipline with claim cleanup
4. stack dependency coordination (branch stacking with `but`)
5. commit-lock recovery via stacked branch alignment and coordination

Run locally:

```bash
cd crates/but-engineering/eval
pnpm install --ignore-workspace
pnpm run eval
```

Optional:

- `pnpm run eval:repeat` for stability sampling
- `pnpm run eval:codex` for non-gating Codex compatibility baseline
- `pnpm run eval:natural` for natural-behavior scenarios
- `pnpm run eval:natural:metrics` for `--repeat 10` metrics + threshold check
- `pnpm run eval:natural:failures` for per-failure missing-signal diagnostics
- `pnpm run eval:natural:diagnose` for repeat + metrics + failure diagnostics in one run
- `pnpm run eval:natural:codex` for non-gating Codex natural-behavior baseline
- `pnpm run eval:composite` for a single demo-style composite coordination scenario
- `pnpm run eval:composite:metrics` for composite repeat metrics (`>= 0.6` target)
- `pnpm run eval:composite:diagnose` for composite repeat + metrics + failure diagnostics

Dual-gate policy:

1. deterministic suite (`promptfooconfig.yaml`) remains the stability gate
2. natural suite (`promptfooconfig.natural.yaml`) tracks emergent coordination;
   default threshold target is `>= 70%` over 10 repeats

For fast tuning of emergent behavior (especially plan/discover discipline), use
`pnpm run eval:natural:diagnose` and iterate on missing-signal output (including stack dependency labels such as `missing_stack_dependency_check` and `missing_stack_anchor_or_alignment`).

### Composite Demo Eval

Use `pnpm run eval:composite` to demo multi-signal cooperation in one run:
independent delivery (`profile` + `parser`), stack dependency coordination, gotcha
discovery posting, and cleanup (`release --all` + `plan --clear`).
Keep this track separate from default gates while tuning skill behavior.

## Design

See [WORKFLOW.md](WORKFLOW.md) for the full specification.
