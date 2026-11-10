#!/usr/bin/env bash
# Lay out the window of a running dev build of the GPUI client, the one the user looks at, as
# asked. Only layout: `src/dev/control.rs` lists the commands. To look at the app without
# touching the user's window, use drive.sh.
#
#   control.sh [COMMAND]...
#       Run each COMMAND in order, one argument each, then print how the window is laid out,
#       after any command's error. With none, only print it.
set -euo pipefail

if [[ -n ${GITBUTLER_GPUI_SOCKET:-} ]]; then
    socket=$GITBUTLER_GPUI_SOCKET
elif [[ -n ${E2E_TEST_APP_DATA_DIR:-} ]]; then
    socket=$E2E_TEST_APP_DATA_DIR/com.gitbutler.app/gpui.sock
elif [[ $(uname) == Darwin ]]; then
    socket="$HOME/Library/Application Support/com.gitbutler.app.dev/gpui.sock"
else
    socket=${XDG_DATA_HOME:-$HOME/.local/share}/com.gitbutler.app.dev/gpui.sock
fi

if [[ ! -S $socket ]]; then
    echo "control.sh: nothing listens at $socket; run a dev build with --features dev-script" >&2
    exit 1
fi
python3 - "$socket" "$@" <<'EOF'
import json, socket, sys

path, commands = sys.argv[1], sys.argv[2:]
request = {"jsonrpc": "2.0", "id": 1, "method": "dev.layout", "params": {"commands": commands}}
with socket.socket(socket.AF_UNIX) as connection:
    connection.connect(path)
    connection.sendall((json.dumps(request) + "\n").encode())
    answer = json.loads(connection.makefile().readline())
if "error" in answer:
    sys.exit("control.sh: " + answer["error"]["message"])
for error in answer["result"]["errors"]:
    print("error: " + error)
print(answer["result"]["layout"])
EOF
