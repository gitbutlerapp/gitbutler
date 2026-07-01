#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

git-init-frozen

commit-file base

git checkout -b document-but-pr-skill
commit-file document-but-pr-skill

git checkout main
git merge --no-ff -m "merge document-but-pr-skill" document-but-pr-skill

setup_target_to_match_main
turn_into_remote_branch document-but-pr-skill document-but-pr-skill
create_workspace_commit_once
