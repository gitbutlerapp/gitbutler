#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

# main has a local commit beyond origin/main, without a managed workspace.
git-init-frozen
commit M
setup_remote_tracking main
add_main_remote_setup
commit local
