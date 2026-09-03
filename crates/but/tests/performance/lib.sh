#!/bin/sh

# Shared support for end-to-end `but` performance scenarios.

PERF_DEFAULT_FIXTURE_REV=cf9f4aad6fe7511d5aeb9fd7c83fc62e18a9e1b6
export PERF_DEFAULT_FIXTURE_REV

perf_die() {
    printf 'performance test error: %s\n' "$*" >&2
    exit 1
}

perf_require_command() {
    command -v "$1" >/dev/null 2>&1 || perf_die "required command not found: $1"
}

perf_assert_full_oid() {
    case "$1" in
        ''|*[!0-9a-f]*) perf_die "fixture revision must be a full lowercase 40-character object ID: $1" ;;
    esac
    [ "${#1}" -eq 40 ] ||
        perf_die "fixture revision must be a full lowercase 40-character object ID: $1"
}

perf_use_run_environment() {
    : "${PERF_RUN_ROOT:?PERF_RUN_ROOT is not set}"

    PERF_REPO=$PERF_RUN_ROOT/repo
    HOME=$PERF_RUN_ROOT/home
    E2E_TEST_APP_DATA_DIR=$PERF_RUN_ROOT/app-data
    TMPDIR=$PERF_RUN_ROOT/tmp

    export PERF_REPO HOME E2E_TEST_APP_DATA_DIR TMPDIR
    export GIT_CONFIG_NOSYSTEM=1
    export GIT_CONFIG_GLOBAL=/dev/null
    export GIT_ATTR_NOSYSTEM=1
    export GIT_TERMINAL_PROMPT=0
    export GIT_AUTHOR_NAME=author
    export GIT_AUTHOR_EMAIL=author@example.com
    export GIT_AUTHOR_DATE='2000-01-01 00:00:00 +0000'
    export GIT_COMMITTER_NAME=committer
    export GIT_COMMITTER_EMAIL=committer@example.com
    export GIT_COMMITTER_DATE='2000-01-02 00:00:00 +0000'
    export TZ=UTC LANG=C LC_ALL=C
    export NO_BG_TASKS=1 NOPAGER=1 PAGER=cat GIT_PAGER=cat
    export BUT_THEME=dark TERM=dumb GITBUTLER_CHANGE_ID=42

    # Deterministic command-level Git settings, matching but-testsupport.
    export GIT_CONFIG_COUNT=5
    export GIT_CONFIG_KEY_0=commit.gpgsign GIT_CONFIG_VALUE_0=false
    export GIT_CONFIG_KEY_1=tag.gpgsign GIT_CONFIG_VALUE_1=false
    export GIT_CONFIG_KEY_2=init.defaultBranch GIT_CONFIG_VALUE_2=main
    export GIT_CONFIG_KEY_3=protocol.file.allow GIT_CONFIG_VALUE_3=always
    export GIT_CONFIG_KEY_4=gitbutler.testing.changeId GIT_CONFIG_VALUE_4=42

    unset GIT_DIR GIT_INDEX_FILE GIT_OBJECT_DIRECTORY
    unset GIT_ALTERNATE_OBJECT_DIRECTORIES GIT_WORK_TREE GIT_COMMON_DIR
    unset GIT_ASKPASS SSH_ASKPASS GIT_EDITOR VISUAL EDITOR
    unset BUT_OUTPUT_FORMAT BUT_PAGER RUST_LOG RUST_BACKTRACE
}

perf_create_gitbutler_fixture() {
    destination=$1
    source_repo=$2
    revision=$3

    perf_assert_full_oid "$revision"
    "$GIT_BIN" -C "$source_repo" cat-file -e "$revision^{commit}" ||
        perf_die "fixture revision does not exist in source repository: $revision"

    "$GIT_BIN" init --quiet --bare "$destination"
    "$GIT_BIN" -C "$destination" fetch --quiet --no-tags "$source_repo" \
        "$revision:refs/heads/main"
    "$GIT_BIN" -C "$destination" symbolic-ref HEAD refs/heads/main
    "$GIT_BIN" -C "$destination" config gc.auto 0
    "$GIT_BIN" -C "$destination" config maintenance.auto false

    # Historical object store and its refs must remain immutable for all samples.
    chmod -R a-w "$destination"
}

perf_reset_run_root() {
    : "${PERF_SESSION_ROOT:?PERF_SESSION_ROOT is not set}"
    : "${PERF_RUN_ROOT:?PERF_RUN_ROOT is not set}"

    case "$PERF_RUN_ROOT" in
        "$PERF_SESSION_ROOT"/*) ;;
        *) perf_die "refusing to reset run root outside performance session: $PERF_RUN_ROOT" ;;
    esac

    rm -rf "$PERF_RUN_ROOT"
    mkdir -p "$PERF_RUN_ROOT"
    perf_use_run_environment
    mkdir -p "$HOME" "$E2E_TEST_APP_DATA_DIR/gitbutler" "$TMPDIR"

    cat >"$E2E_TEST_APP_DATA_DIR/gitbutler/settings.json" <<'EOF'
{
  "agentSkillNotices": false,
  "appUpdatesCheckIntervalSec": 0,
  "contextLines": 3,
  "fetch": { "autoFetchIntervalMinutes": -1 },
  "telemetry": {
    "appMetricsEnabled": false,
    "appErrorReportingEnabled": false,
    "migratedFromLegacy": true
  }
}
EOF
}

perf_create_gitbutler_workspace() {
    destination=$1
    target_revision=${2:-}
    : "${PERF_FIXTURE_REPO:?PERF_FIXTURE_REPO is not set}"
    : "${BUT_BIN:?BUT_BIN is not set}"

    remote=$PERF_RUN_ROOT/remote.git
    "$GIT_BIN" clone --quiet --bare --shared "$PERF_FIXTURE_REPO" "$remote"
    "$GIT_BIN" -C "$remote" config gc.auto 0
    "$GIT_BIN" -C "$remote" config maintenance.auto false
    if [ -n "$target_revision" ]; then
        "$GIT_BIN" -C "$remote" cat-file -e "$target_revision^{commit}" ||
            perf_die "workspace target does not exist in fixture: $target_revision"
        "$GIT_BIN" -C "$remote" update-ref refs/heads/main "$target_revision"
    fi

    "$GIT_BIN" clone --quiet --shared "$remote" "$destination"
    "$GIT_BIN" -C "$destination" config gc.auto 0
    "$GIT_BIN" -C "$destination" config maintenance.auto false

    setup_log=$PERF_RUN_ROOT/but-setup.log
    if ! "$BUT_BIN" -C "$destination" setup >"$setup_log" 2>&1; then
        cat "$setup_log" >&2
        perf_die "but setup failed"
    fi
}

perf_git() {
    : "${PERF_REPO:?PERF_REPO is not set}"
    "$GIT_BIN" -C "$PERF_REPO" "$@"
}

perf_but() {
    : "${PERF_REPO:?PERF_REPO is not set}"
    "$BUT_BIN" -C "$PERF_REPO" "$@"
}

perf_exec_but() {
    : "${PERF_REPO:?PERF_REPO is not set}"
    if [ "${PERF_SHOW_OUTPUT:-0}" = 1 ]; then
        exec "$BUT_BIN" -C "$PERF_REPO" "$@"
    else
        exec "$BUT_BIN" -C "$PERF_REPO" "$@" >/dev/null
    fi
}

perf_state_begin() {
    : "${PERF_RUN_ROOT:?PERF_RUN_ROOT is not set}"
    PERF_STATE_TMP=$PERF_RUN_ROOT/scenario.env.tmp
    export PERF_STATE_TMP
    : >"$PERF_STATE_TMP"
}

perf_state_set() {
    name=$1
    value=$2
    : "${PERF_STATE_TMP:?call perf_state_begin before perf_state_set}"

    case "$name" in
        ''|[0-9]*|*[!A-Z0-9_]*) perf_die "invalid scenario state name: $name" ;;
    esac

    quoted_value=$(printf '%s' "$value" | sed "s/'/'\\\\''/g")
    printf "%s='%s'\n" "$name" "$quoted_value" >>"$PERF_STATE_TMP"
}

perf_state_commit() {
    : "${PERF_STATE_TMP:?call perf_state_begin before perf_state_commit}"
    mv "$PERF_STATE_TMP" "$PERF_RUN_ROOT/scenario.env"
    unset PERF_STATE_TMP
}

perf_state_load() {
    state_file=$PERF_RUN_ROOT/scenario.env
    [ -f "$state_file" ] || perf_die "scenario state not found: $state_file"
    # Scenario files are trusted repository code. perf_state_set() quotes dynamic values.
    # shellcheck disable=SC1090
    . "$state_file"
    unset state_file
}
