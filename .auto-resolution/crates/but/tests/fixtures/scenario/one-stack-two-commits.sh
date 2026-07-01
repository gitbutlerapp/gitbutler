#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

git-init-frozen
commit-file Base
setup_target_to_match_main

git checkout -b A
commit-file first
commit-file second

create_workspace_commit_once A
