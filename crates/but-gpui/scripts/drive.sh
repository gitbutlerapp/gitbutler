#!/usr/bin/env bash
# Drive the GPUI client from a script and save screenshots of it, in a window that never
# shows, against app data of its own. `src/dev.rs` lists the commands a script takes, and
# `.agents/skills/gpui-screenshots/SKILL.md` says how to use this.
#
#   drive.sh run [--release] [--timeout SECS] SCRIPT
#       Build, run SCRIPT (- for stdin) to its end, then print what the app reported.
#       Killed after --timeout seconds, 300 by default.
#
#   drive.sh start [--release] [--timeout SECS]
#       Build and start a session, which `step` drives a step at a time and which quits
#       after --timeout seconds, 1800 by default. One per checkout.
#   drive.sh step [COMMAND]...
#       Run each COMMAND, one script line per argument, then save a shot and print its path,
#       after anything the app reported meanwhile and what has the keys (KEYS).
#   drive.sh stop
#       Quit the session.
set -euo pipefail

root=$(cd "$(dirname "$0")/../../.." && pwd)
target=${CARGO_TARGET_DIR:-$root/target}
session=$target/gpui-drive

die() {
    echo "drive.sh: $*" >&2
    exit 1
}

# What's worth reading in the app's output: the script's own reports (PERF, PROMPT, failed
# steps) and the log's warnings and errors, without its info.
report() {
    grep -v ' INFO ' || true
}

build() {
    local profile=$1
    local args=(build -q -p but-gpui --features dev-script,gpui_platform/runtime_shaders)
    if [[ $profile == release ]]; then
        args+=(--release)
    fi
    (cd "$root" && cargo "${args[@]}") || die "the build failed"
    bin=$target/$profile/gitbutler-gpui
}

# Kill `pid` once it has run `timeout` seconds. Checks every second, so it ends with it.
guard() {
    local pid=$1 timeout=$2
    (
        for ((i = 0; i < timeout; i++)); do
            kill -0 "$pid" 2>/dev/null || exit 0
            sleep 1
        done
        kill "$pid" 2>/dev/null && echo "drive.sh: killed the app after ${timeout}s" >&2
    ) &
}

running() {
    [[ -f $session/pid ]] && kill -0 "$(cat "$session/pid")" 2>/dev/null
}

cmd_run() {
    local profile=debug timeout=300 script=
    while (($#)); do
        case $1 in
            --release) profile=release ;;
            --timeout) timeout=$2 && shift ;;
            *) script=$1 ;;
        esac
        shift
    done
    [[ -n $script ]] || die "run takes a script, or - for stdin"
    build "$profile"
    local dir
    dir=$(mktemp -d "${TMPDIR:-/tmp}/gpui-drive.XXXXXX")
    mkdir "$dir/data"
    cat -- "$script" >"$dir/script"
    E2E_TEST_APP_DATA_DIR=$dir/data GITBUTLER_GPUI_SCRIPT=$dir/script \
        "$bin" >"$dir/out.log" 2>&1 &
    local pid=$! status=0
    guard "$pid" "$timeout"
    wait "$pid" || status=$?
    report <"$dir/out.log"
    echo "drive.sh: app data and logs in $dir" >&2
    return "$status"
}

cmd_start() {
    local profile=debug timeout=1800
    while (($#)); do
        case $1 in
            --release) profile=release ;;
            --timeout) timeout=$2 && shift ;;
            *) die "start takes --release and --timeout" ;;
        esac
        shift
    done
    if running; then
        die "a session is running (pid $(cat "$session/pid")); stop it first"
    fi
    build "$profile"
    rm -rf "${session:?}"
    mkdir -p "$session/shots" "$session/data"
    mkfifo "$session/pipe"
    E2E_TEST_APP_DATA_DIR=$session/data GITBUTLER_GPUI_SCRIPT=$session/pipe \
        nohup "$bin" >"$session/out.log" 2>&1 &
    echo $! >"$session/pid"
    # Holds the pipe open between steps: the app quits once no one has it open, so this
    # going away ends the session.
    nohup sleep "$timeout" >"$session/pipe" 2>/dev/null &
    echo $! >"$session/holder"
    echo "drive.sh: started; shots go to $session/shots" >&2
}

cmd_step() {
    running || die "no session; start one with drive.sh start"
    local n
    n=$(($(cat "$session/n" 2>/dev/null || echo 0) + 1))
    echo "$n" >"$session/n"
    local shot
    shot=$session/shots/$(printf %03d "$n").png
    local offset
    offset=$(wc -c <"$session/out.log")
    {
        for command in "$@"; do
            printf '%s\n' "$command"
        done
        echo keys
        echo "shot $shot"
        # Steps run in order, so once this runs the shot is written.
        echo "sh touch '$shot.done'"
    } >"$session/pipe"
    local i
    for ((i = 0; i < 1200; i++)); do
        [[ -f $shot.done ]] && break
        running || break
        sleep 0.1
    done
    tail -c +$((offset + 1)) "$session/out.log" | report
    if [[ -f $shot.done ]]; then
        rm "$shot.done"
        echo "$shot"
    elif running; then
        die "the step didn't finish within 2 minutes"
    else
        die "the app quit; its log is $session/out.log"
    fi
}

cmd_stop() {
    running || die "no session"
    local pid
    pid=$(cat "$session/pid")
    kill "$(cat "$session/holder")" 2>/dev/null || true
    # It quits once operations under way finish.
    local i
    for ((i = 0; i < 300; i++)); do
        kill -0 "$pid" 2>/dev/null || break
        sleep 0.1
    done
    kill "$pid" 2>/dev/null || true
    rm "$session/pid"
    echo "drive.sh: stopped; app data and logs in $session/data" >&2
}

command=${1:-}
shift || true
case $command in
    run) cmd_run "$@" ;;
    start) cmd_start "$@" ;;
    step) cmd_step "$@" ;;
    stop) cmd_stop "$@" ;;
    *) die "usage: drive.sh run|start|step|stop (see the top of this file)" ;;
esac
