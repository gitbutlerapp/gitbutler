#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

# origin/main has a remote commit beyond main, which remains checked out.
git-init-frozen
commit M
git checkout -b remote
commit remote
git checkout main
turn_into_remote_branch remote main
add_main_remote_setup
