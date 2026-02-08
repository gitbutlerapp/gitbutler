#!/usr/bin/env bash
set -euo pipefail

CLAUDE_BIN="${BUT_EVAL_CLAUDE_BIN:-${BUT_EVAL_RUNNER_BIN:-claude}}"
PROMPT="${BUT_EVAL_PROMPT:-}"
MODEL="${BUT_EVAL_MODEL:-}"
ALLOWED_TOOLS="${BUT_EVAL_ALLOWED_TOOLS:-Bash,Read,Edit,Write,Glob,Grep,LS,MultiEdit,TodoWrite}"
PERMISSION_MODE="${BUT_EVAL_PERMISSION_MODE:-bypassPermissions}"
APPEND_SYSTEM_PROMPT="${BUT_EVAL_APPEND_SYSTEM_PROMPT:-}"
AUTH_MODE="${BUT_EVAL_AUTH_MODE:-auto}"
API_KEY="${BUT_EVAL_ANTHROPIC_API_KEY:-${ANTHROPIC_API_KEY:-}}"
MIN_CLAUDE_VERSION="${BUT_EVAL_MIN_CLAUDE_VERSION:-${BUT_EVAL_MIN_RUNNER_VERSION:-1.0.88}}"

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

declare -a claude_candidates=()
if [[ "$CLAUDE_BIN" == */* ]]; then
  claude_candidates+=("$CLAUDE_BIN")
else
  if command -v "$CLAUDE_BIN" >/dev/null 2>&1; then
    while IFS= read -r candidate; do
      [[ -n "$candidate" ]] && claude_candidates+=("$candidate")
    done < <(which -a "$CLAUDE_BIN" 2>/dev/null | awk '!seen[$0]++')
  fi
fi

if [[ "${#claude_candidates[@]}" -eq 0 ]]; then
  echo "Claude CLI not found: $CLAUDE_BIN" >&2
  exit 2
fi

SELECTED_CLAUDE_BIN=""
SELECTED_CLAUDE_VERSION=""
CANDIDATE_SUMMARY=""
for candidate in "${claude_candidates[@]}"; do
  if [[ ! -x "$candidate" ]]; then
    continue
  fi

  version_raw="$("$candidate" --version 2>&1 || true)"
  version="$(extract_semver "$version_raw")"
  if [[ -z "$version" ]]; then
    CANDIDATE_SUMMARY+="$candidate: unparseable version ($version_raw)\n"
    continue
  fi

  CANDIDATE_SUMMARY+="$candidate: $version\n"

  if semver_gte "$version" "$MIN_CLAUDE_VERSION"; then
    if [[ -z "$SELECTED_CLAUDE_VERSION" ]] || semver_gte "$version" "$SELECTED_CLAUDE_VERSION"; then
      SELECTED_CLAUDE_BIN="$candidate"
      SELECTED_CLAUDE_VERSION="$version"
    fi
  fi
done

if [[ -z "$SELECTED_CLAUDE_BIN" ]]; then
  printf 'No Claude CLI binary satisfies >= %s.\nChecked:\n%b' "$MIN_CLAUDE_VERSION" "$CANDIDATE_SUMMARY" >&2
  exit 2
fi

CLAUDE_BIN="$SELECTED_CLAUDE_BIN"

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
