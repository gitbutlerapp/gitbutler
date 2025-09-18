#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

### General Description

# A ws-ref points to a ws-commit, with an empty stack inside, but the workspace ref is advanced by an outside commit.
git init
commit M1

git branch B
git checkout -b A

create_workspace_commit_once A
commit O1
