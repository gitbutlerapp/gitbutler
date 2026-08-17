#!/bin/bash

set -euo pipefail

action="$1"
directory="$2"

case "$action" in
  undo|redo) ;;
  *) echo "Expected undo or redo" >&2; exit 2 ;;
esac

pushd "$directory"
"$BUT" "$action"
popd
