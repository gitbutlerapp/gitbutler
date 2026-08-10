#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

# The checked-out topic branch has no commits outside the stored target. Its
# tracking tip is reachable from the target, which is the evidence that the
# otherwise-empty branch is integrated.
git init
commit-file base.txt base

git checkout -b topic
commit-file topic.txt topic
setup_remote_tracking topic
git config branch.topic.remote origin
git config branch.topic.merge refs/heads/topic

git checkout main
git merge --no-ff -m "merge topic" topic
setup_target_to_match_main

git checkout -b advanced-target
git commit --allow-empty -m "upstream commit"
git update-ref refs/remotes/origin/main advanced-target

git checkout topic
git branch -D advanced-target
