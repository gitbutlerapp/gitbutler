#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="${BUT_EVAL_REPO_ROOT:-$(cd "$SCRIPT_DIR/../../../../" && pwd)}"
BUT_BIN="${BUT_EVAL_BUT_BIN:-$REPO_ROOT/target/debug/but}"
SKILL_INSTALL_PATHS_RAW="${BUT_EVAL_SKILL_INSTALL_PATHS:-.claude/skills/gitbutler,.codex/skills/gitbutler}"

if [[ ! -x "$BUT_BIN" ]]; then
  cargo build -p but --manifest-path "$REPO_ROOT/Cargo.toml"
fi

FIXTURE_DIR="$(mktemp -d)"
FIXTURE_DIR="$(cd "$FIXTURE_DIR" && pwd -P)"
KEEP_FIXTURES="${BUT_EVAL_KEEP_FIXTURES:-0}"
cleanup_fixture() {
  local exit_code=$?
  if [[ "$exit_code" -ne 0 && "$KEEP_FIXTURES" != "1" && -n "${FIXTURE_DIR:-}" && -d "$FIXTURE_DIR" ]]; then
    rm -rf "$FIXTURE_DIR"
  fi
}
trap cleanup_fixture ERR EXIT

APP_DATA_DIR="$FIXTURE_DIR/.but-data"
git init --initial-branch=main "$FIXTURE_DIR" >/dev/null
git -C "$FIXTURE_DIR" config user.name "Tier4 Eval"
git -C "$FIXTURE_DIR" config user.email "tier4-eval@example.com"
git -C "$FIXTURE_DIR" commit --allow-empty -m "init" >/dev/null
mkdir -p "$APP_DATA_DIR"
{
  echo ".but-data/"
  echo ".claude/"
  echo ".codex/"
  echo ".tmp/"
} >>"$FIXTURE_DIR/.git/info/exclude"

E2E_TEST_APP_DATA_DIR="$APP_DATA_DIR" "$BUT_BIN" -C "$FIXTURE_DIR" setup >/dev/null
IFS=',' read -r -a SKILL_INSTALL_PATHS <<<"$SKILL_INSTALL_PATHS_RAW"
for skill_path in "${SKILL_INSTALL_PATHS[@]}"; do
  trimmed_path="$skill_path"
  trimmed_path="${trimmed_path#"${trimmed_path%%[![:space:]]*}"}"
  trimmed_path="${trimmed_path%"${trimmed_path##*[![:space:]]}"}"
  if [[ -n "$trimmed_path" ]]; then
    E2E_TEST_APP_DATA_DIR="$APP_DATA_DIR" "$BUT_BIN" -C "$FIXTURE_DIR" skill install --path "$trimmed_path" >/dev/null
  fi
done
if ! E2E_TEST_APP_DATA_DIR="$APP_DATA_DIR" "$BUT_BIN" -C "$FIXTURE_DIR" status --json >/dev/null 2>&1; then
  echo "Failed to initialize GitButler setup in fixture: $FIXTURE_DIR" >&2
  exit 1
fi

trap - ERR EXIT
echo "$FIXTURE_DIR"
