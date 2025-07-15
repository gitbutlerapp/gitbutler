#!/usr/bin/env bash

### General Description

# Various stages through a typical user journey. Ideally complete enough to test everything that matters to us.
set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

git init 01-unborn
(cd 01-unborn
  echo "a newly initialized repository" >.git/description
)

git init 02-first-commit
(cd 02-first-commit
  echo "the root commit is now present locally" >.git/description
  commit init
)

cp -R 02-first-commit 03-main-pushed
(cd 03-main-pushed
  cat <<EOF >.git/description
main was pushed so it can now serve as target.

However, without an official workspace it still won't be acting as a target.
EOF
  setup_target_to_match_main
)

cp -R 03-main-pushed 04-create-workspace
(cd 04-create-workspace
  echo "An official workspace was created, with nothing in it" >.git/description
  create_workspace_commit_once main
)

cp -R 04-create-workspace 05-empty-stack
(cd 05-empty-stack
  echo "an empty stack with nothing in it" >.git/description

  git branch S1 gitbutler/workspace~1
)

git init 06-create-commit-in-stack
(cd 06-create-commit-in-stack
  echo "Create a new commit in the newly added stack S1" >.git/description
  commit init && setup_target_to_match_main
  git checkout -b S1
    commit one
  create_workspace_commit_once S1
)

cp -R 06-create-commit-in-stack 07-push-commit
(cd 07-push-commit
  echo "push S1 to the remote which is then up-to-date" >.git/description
  git rev-parse S1 > .git/refs/remotes/origin/S1
)

git init 08-new-local-commit
(cd 08-new-local-commit
  echo "Create a new local commit right after the previous pushed one

  This leaves the stack in a state where it can be pushed.
  " >.git/description
  commit init && setup_target_to_match_main
  git checkout -b S1
    commit one
    git rev-parse @ > .git/refs/remotes/origin/S1
    commit two
  create_workspace_commit_once S1
)

git init 09-rewritten-local-commit
(cd 09-rewritten-local-commit
  echo "The new local commit was rewritten after pushing it to the remote" >.git/description
  commit init && setup_target_to_match_main
  git checkout -b S1
    commit one
    git branch soon-S1-remote
    echo hi>file && git add file && git commit -am two
  git checkout soon-S1-remote
    tick
    git cherry-pick S1
    setup_remote_tracking soon-S1-remote S1 "move"
  git checkout S1
  create_workspace_commit_once S1
)
