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
MIN_CLAUDE_VERSION="${BUT_EVAL_MIN_CLAUDE_VERSION:-1.0.88}"

extract_semver() {
  local raw="$1"
  local semver
  semver="$(printf '%s\n' "$raw" | grep -Eo '[0-9]+\.[0-9]+\.[0-9]+' | head -n 1 || true)"
  printf '%s' "$semver"
}

semver_gte() {
  local left="$1"
  local right="$2"
  local l1 l2 l3 r1 r2 r3
  IFS=. read -r l1 l2 l3 <<<"$left"
  IFS=. read -r r1 r2 r3 <<<"$right"

  l1="${l1:-0}"
  l2="${l2:-0}"
  l3="${l3:-0}"
  r1="${r1:-0}"
  r2="${r2:-0}"
  r3="${r3:-0}"

  if ((l1 != r1)); then
    ((l1 > r1))
    return
  fi
  if ((l2 != r2)); then
    ((l2 > r2))
    return
  fi
  ((l3 >= r3))
}

if [[ -z "$PROMPT" ]]; then
  echo "BUT_EVAL_PROMPT is required" >&2
  exit 2
fi

if ! command -v "$CLAUDE_BIN" >/dev/null 2>&1; then
  echo "Claude CLI not found: $CLAUDE_BIN" >&2
  exit 2
fi

CLAUDE_PATH="$(command -v "$CLAUDE_BIN")"
CLAUDE_VERSION_RAW="$("$CLAUDE_BIN" --version 2>&1 || true)"
CLAUDE_VERSION="$(extract_semver "$CLAUDE_VERSION_RAW")"
if [[ -z "$CLAUDE_VERSION" ]]; then
  echo "Could not parse Claude CLI version from '$CLAUDE_VERSION_RAW' ($CLAUDE_PATH)." >&2
  exit 2
fi

if ! semver_gte "$CLAUDE_VERSION" "$MIN_CLAUDE_VERSION"; then
  echo "Claude CLI version $CLAUDE_VERSION at $CLAUDE_PATH is too old; need >= $MIN_CLAUDE_VERSION." >&2
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
  --verbose
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
