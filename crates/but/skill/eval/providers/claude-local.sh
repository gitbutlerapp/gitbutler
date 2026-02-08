#!/usr/bin/env bash
set -euo pipefail

CLAUDE_BIN="${BUT_EVAL_CLAUDE_BIN:-claude}"
PROMPT="${BUT_EVAL_PROMPT:-}"
MODEL="${BUT_EVAL_MODEL:-}"
ALLOWED_TOOLS="${BUT_EVAL_ALLOWED_TOOLS:-Bash,Read,Edit,Write,Glob,Grep,LS,MultiEdit,TodoWrite}"
PERMISSION_MODE="${BUT_EVAL_PERMISSION_MODE:-bypassPermissions}"
APPEND_SYSTEM_PROMPT="${BUT_EVAL_APPEND_SYSTEM_PROMPT:-}"
AUTH_MODE="${BUT_EVAL_AUTH_MODE:-auto}"
API_KEY="${BUT_EVAL_ANTHROPIC_API_KEY:-${ANTHROPIC_API_KEY:-}}"

if [[ -z "$PROMPT" ]]; then
  echo "BUT_EVAL_PROMPT is required" >&2
  exit 2
fi

if ! command -v "$CLAUDE_BIN" >/dev/null 2>&1; then
  echo "Claude CLI not found: $CLAUDE_BIN" >&2
  exit 2
fi

case "$AUTH_MODE" in
  local)
    # Force Claude Code account auth.
    unset ANTHROPIC_API_KEY
    ;;
  api)
    if [[ -z "$API_KEY" ]]; then
      echo "API auth mode requires ANTHROPIC_API_KEY (or BUT_EVAL_ANTHROPIC_API_KEY)." >&2
      exit 2
    fi
    export ANTHROPIC_API_KEY="$API_KEY"
    ;;
  auto)
    if [[ -n "$API_KEY" ]]; then
      export ANTHROPIC_API_KEY="$API_KEY"
    else
      unset ANTHROPIC_API_KEY
    fi
    ;;
  *)
    echo "Invalid BUT_EVAL_AUTH_MODE: $AUTH_MODE (expected: auto, local, api)" >&2
    exit 2
    ;;
esac

args=(
  -p "$PROMPT"
  --output-format stream-json
  --permission-mode "$PERMISSION_MODE"
  --dangerously-skip-permissions
  --allowedTools "$ALLOWED_TOOLS"
)

if [[ -n "$MODEL" ]]; then
  args+=(--model "$MODEL")
fi

if [[ -n "$APPEND_SYSTEM_PROMPT" ]]; then
  args+=(--append-system-prompt "$APPEND_SYSTEM_PROMPT")
fi

"$CLAUDE_BIN" "${args[@]}"
