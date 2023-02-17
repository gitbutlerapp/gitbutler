#!/bin/bash

set -o errexit
set -o nounset
set -o pipefail

PWD="$(dirname $(readlink -f -- $0))"

pushd "$PWD/../src-tauri" >/dev/null
PKGID="$(cargo pkgid)"
popd >/dev/null

echo -n "$PKGID" | cut -d'@' -f2
