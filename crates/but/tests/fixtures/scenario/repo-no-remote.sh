#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

### General Description

# A repository with commits but no remote configured
# This tests that `but setup` creates a gb-local remote
git-init-frozen
commit M
