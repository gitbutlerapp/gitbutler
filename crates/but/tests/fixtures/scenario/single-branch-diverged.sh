#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

# main and origin/main each have a distinct commit beyond their shared base.
git-init-frozen
commit M
git checkout -b remote
commit remote
git checkout main
turn_into_remote_branch remote main
add_main_remote_setup
commit local
