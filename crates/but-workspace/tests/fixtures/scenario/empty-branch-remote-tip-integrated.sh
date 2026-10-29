#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

git init

commit-file base

git checkout -b topic
commit-file topic

git checkout main
git merge --no-ff -m "merge topic" topic

setup_target_to_match_main
turn_into_remote_branch topic topic
git config branch.topic.remote origin
git config branch.topic.merge refs/heads/topic
git checkout -b topic origin/topic
create_workspace_commit_once topic
