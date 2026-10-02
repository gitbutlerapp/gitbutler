#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

# main and origin/main point to the same commit, without a managed workspace.
git-init-frozen
commit M
setup_remote_tracking main
add_main_remote_setup
