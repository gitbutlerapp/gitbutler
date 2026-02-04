#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="${BUT_EVAL_REPO_ROOT:-$(cd "$SCRIPT_DIR/../../../" && pwd)}"
ENGINEERING_BIN="${BUT_EVAL_ENGINEERING_BIN:-$REPO_ROOT/target/debug/but-engineering}"
BUT_BIN="${BUT_EVAL_BUT_BIN:-$REPO_ROOT/target/debug/but}"
SKILL_SOURCE="$REPO_ROOT/crates/but-engineering/skill/SKILL.md"
BUT_SKILL_SOURCE="$REPO_ROOT/crates/but/skill/SKILL.md"
HOOK_SETTINGS_SOURCE="$REPO_ROOT/crates/but-engineering/examples/hook-settings.json"

if [[ ! -x "$ENGINEERING_BIN" ]]; then
  cargo build -p but-engineering --manifest-path "$REPO_ROOT/Cargo.toml"
fi
if [[ ! -x "$BUT_BIN" ]]; then
  cargo build -p but --manifest-path "$REPO_ROOT/Cargo.toml"
fi

if [[ ! -f "$SKILL_SOURCE" ]]; then
  echo "Skill source not found: $SKILL_SOURCE" >&2
  exit 1
fi
if [[ ! -f "$BUT_SKILL_SOURCE" ]]; then
  echo "Skill source not found: $BUT_SKILL_SOURCE" >&2
  exit 1
fi

if [[ ! -f "$HOOK_SETTINGS_SOURCE" ]]; then
  echo "Hook settings source not found: $HOOK_SETTINGS_SOURCE" >&2
  exit 1
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

git init --initial-branch=main "$FIXTURE_DIR" >/dev/null
git -C "$FIXTURE_DIR" config user.name "Tier4 Eval"
git -C "$FIXTURE_DIR" config user.email "tier4-eval@example.com"
git -C "$FIXTURE_DIR" commit --allow-empty -m "init" >/dev/null

APP_DATA_DIR="$FIXTURE_DIR/.but-data"
mkdir -p "$APP_DATA_DIR"
E2E_TEST_APP_DATA_DIR="$APP_DATA_DIR" "$BUT_BIN" -C "$FIXTURE_DIR" setup >/dev/null
if ! E2E_TEST_APP_DATA_DIR="$APP_DATA_DIR" "$BUT_BIN" -C "$FIXTURE_DIR" status --json >/dev/null 2>&1; then
  echo "GitButler preflight failed: but status --json is unavailable after setup" >&2
  exit 1
fi

mkdir -p "$FIXTURE_DIR/.claude/skills/but-engineering"
mkdir -p "$FIXTURE_DIR/.codex/skills/but-engineering"
mkdir -p "$FIXTURE_DIR/.claude/skills/gitbutler"
mkdir -p "$FIXTURE_DIR/.codex/skills/gitbutler"
cp "$SKILL_SOURCE" "$FIXTURE_DIR/.claude/skills/but-engineering/SKILL.md"
cp "$SKILL_SOURCE" "$FIXTURE_DIR/.codex/skills/but-engineering/SKILL.md"
cp "$BUT_SKILL_SOURCE" "$FIXTURE_DIR/.claude/skills/gitbutler/SKILL.md"
cp "$BUT_SKILL_SOURCE" "$FIXTURE_DIR/.codex/skills/gitbutler/SKILL.md"
cp "$HOOK_SETTINGS_SOURCE" "$FIXTURE_DIR/.claude/settings.json"

mkdir -p "$FIXTURE_DIR/.tmp"
{
  echo ".but-data/"
  echo ".claude/"
  echo ".codex/"
  echo ".tmp/"
} >>"$FIXTURE_DIR/.git/info/exclude"

trap - ERR EXIT
echo "$FIXTURE_DIR"
