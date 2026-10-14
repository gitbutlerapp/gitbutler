#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

### General Description

# A repository with a remote configured and remote/HEAD set
# A Git repository with an existing remote HEAD.
git-init-frozen
commit M

# Set up remote tracking for main with HEAD
setup_target_to_match_main
