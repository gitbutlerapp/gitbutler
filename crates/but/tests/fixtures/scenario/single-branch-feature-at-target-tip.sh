#!/usr/bin/env bash

set -eu -o pipefail
source "${BASH_SOURCE[0]%/*}/shared.sh"

git-init-frozen
commit M
setup_remote_tracking main
add_main_remote_setup

git checkout -b feature
commit feature
git update-ref refs/remotes/origin/main HEAD
