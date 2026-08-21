#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

### General Description

# A repo in single branch mode whose main and origin/main tips match.

git-init-frozen

commit-file init

git checkout main
  commit M

setup_target_to_match_main
