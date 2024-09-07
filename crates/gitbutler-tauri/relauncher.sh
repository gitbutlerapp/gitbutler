#!/bin/bash
# This relauncher is necessary for Tauri v1 -> v2 upgrades. The binary name
# has changed from e.g. `GitButler` to `gitbutler-tauri`, but on restarting
# the app the executable is expected to have the same path. This bash script
# is copied into the known location of the old binary name.

SCRIPT_DIR=$( cd -- "$( dirname -- "${BASH_SOURCE[0]}" )" &> /dev/null && pwd )

# Line needs to be quoted since $SCRIPT_DIR contains spaces.
"$SCRIPT_DIR/gitbutler-tauri"
