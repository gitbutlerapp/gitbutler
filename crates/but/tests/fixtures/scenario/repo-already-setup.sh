#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

### General Description

# A repository that has already been set up with GitButler
# This tests that `but setup` recognizes it's already configured
git-init-frozen
commit M

# Set up remote tracking with HEAD (standard setup)
setup_target_to_match_main
