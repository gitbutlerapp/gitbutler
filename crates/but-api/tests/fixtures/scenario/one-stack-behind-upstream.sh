#!/usr/bin/env bash

# A workspace with one stack that has upstream changes.
#
# master: M1 -> M2  (origin/main points to M2)
# feature-branch: M1 -> feature_commit
# gitbutler/workspace: on top of feature-branch
#
# Tests set default_target.sha = M1, so origin/main (at M2) is ahead.

source "${BASH_SOURCE[0]%/*}/shared.sh"

git init -b main

commit M1
git branch M1

commit M2

setup_target_to_match_main

git checkout M1
git checkout -b feature-branch
  echo "feature work" > feature-file.txt
  git add feature-file.txt
  git commit -m "feature commit"

create_workspace_commit_once feature-branch
