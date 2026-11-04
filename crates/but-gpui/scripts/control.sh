#!/usr/bin/env bash
# Lay out the window of a running dev build of the GPUI client, the one the user looks at, as
# asked. Only layout: `src/dev/control.rs` lists the commands. To look at the app without
# touching the user's window, use drive.sh.
#
#   control.sh [COMMAND]...
#       Run each COMMAND in order, one argument each, then print how the window is laid out,
#       after any command's error. With none, only print it.
#
# A build with `--features dev-script`, not driven by a script, listens at
# $GITBUTLER_GPUI_CONTROL, else gitbutler-gpui.sock in the temporary folder.
set -euo pipefail

socket=${GITBUTLER_GPUI_CONTROL:-${TMPDIR:-/tmp}}
[[ -n ${GITBUTLER_GPUI_CONTROL:-} ]] || socket=${socket%/}/gitbutler-gpui.sock

if [[ ! -S $socket ]]; then
    echo "control.sh: nothing listens at $socket; run a dev build with --features dev-script" >&2
    exit 1
fi
# The empty line ends the request; the app answers and closes the connection.
{
    printf '%s\n' "$@"
    echo
} | nc -U "$socket"
