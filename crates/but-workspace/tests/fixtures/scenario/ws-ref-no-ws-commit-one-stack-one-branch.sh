#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

### General Description

# A ws-ref points to a commit, which is also owned by a stack. There is another branch outside of the workspace pointing to the same commit.
git init
commit A
git branch A
git branch B
git checkout -b gitbutler/workspace
