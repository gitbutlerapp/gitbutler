#!/bin/sh
set -eu

: "${PERF_ROOT:?PERF_ROOT is not set}"
# shellcheck disable=SC1091
. "$PERF_ROOT/lib.sh"
perf_use_run_environment

if [ "${1:-}" = --inside-pty ]; then
    stty rows 40 cols 120
    export TERM=xterm-256color
    # Keep but's stdout attached to the PTY, including when using a profiler.
    export PERF_SHOW_OUTPUT=1
    perf_exec_but tui --quit-after 0
fi

perf_state_load
# Suppress output outside the PTY rather than in perf_exec_but: but requires
# terminal I/O. script's transcript is discarded regardless of output mode.
if [ "${PERF_SHOW_OUTPUT:-0}" != 1 ]; then
    exec >/dev/null
fi
case "$PTY_PLATFORM" in
    Linux)
        # Expand the path in the child shell, not in script's command string.
        export SHELL=/bin/sh
        # shellcheck disable=SC2016
        exec script -q -e -c 'exec "$PERF_ROOT/scenarios/tui-startup/test.sh" --inside-pty' /dev/null
        ;;
    Darwin)
        # macOS script takes argv directly and propagates child exit status.
        exec script -q /dev/null "$PERF_ROOT/scenarios/tui-startup/test.sh" --inside-pty
        ;;
    *) perf_die "unsupported PTY platform: $PTY_PLATFORM" ;;
esac
