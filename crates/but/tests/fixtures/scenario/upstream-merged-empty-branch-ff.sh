#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

git-init-frozen

commit-file base
git update-ref refs/heads/base HEAD

git checkout -b document-but-pr-skill
commit-file document-but-pr-skill

git checkout main
git merge --ff-only document-but-pr-skill

setup_target_to_match_main
turn_into_remote_branch document-but-pr-skill document-but-pr-skill
create_workspace_commit_once
