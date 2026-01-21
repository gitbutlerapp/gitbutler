#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

### General Description

# A repository with no remote and branch name is neither main nor master
# This tests that `but setup` handles non-standard default branches
git-init-frozen
git checkout -b development
commit M
