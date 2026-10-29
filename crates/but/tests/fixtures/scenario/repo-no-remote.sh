#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

### General Description

# A repository with commits but no remote configured
# A normal Git repository without any remotes.
git-init-frozen
commit M
