#!/usr/bin/env bash

### General Description

# Various directories with different scenarios for testing stack information *with* a workspace commit,
# and of course with a remote and a branch to integrate with.
set -eu -o pipefail

function set_author() {
  local author=${1:?Author}

  unset GIT_AUTHOR_NAME
  unset GIT_AUTHOR_EMAIL

  git config user.name $author
  git config user.email $author@example.com
}


# can only be called once per test setup
function create_workspace_commit_once() {
  local workspace_commit_subject="GitButler Workspace Commit"

  if [ $# == 1 ]; then
    local current_branch=$(git rev-parse --abbrev-ref HEAD)
    if [[ "$current_branch" != "$1" ]]; then
      echo "BUG: Must assure the current branch is the branch passed as argument: $current_branch != $1"
      return 42
    fi
  fi

  git checkout -b gitbutler/workspace
  if [ $# == 1 ] || [ $# == 0 ]; then
    git commit --allow-empty -m "$workspace_commit_subject"
  else
    git merge -m "$workspace_commit_subject" "${@}"
  fi
}

git init remote
(cd remote
  touch file
  git add . && git commit -m init-integration

  git checkout -b A
  touch file-in-A && git add . && git commit -m "new file in A"
  echo change >file-in-A && git commit -am "change in A"

  git checkout main
)

# The remote has a new commit, but is fast-forwardable
git clone remote remote-advanced-ff
(cd remote-advanced-ff
  git checkout -b A origin/A
  git reset --hard @~1

  create_workspace_commit_once A
)

# There are multiple stacked branches that could lead towards a shared stack.
git clone remote multiple-stacks-with-shared-segment
(cd multiple-stacks-with-shared-segment
  git checkout -b A origin/A
  git reset --hard @~1

set -x
  git checkout -b B-on-A
  echo >new-in-B && git add . && git commit -am "add new file in B-on-A"

  git checkout -b C-on-A A
  echo >new-in-C && git add . && git commit -am "add new file in C-on-A"

  create_workspace_commit_once B-on-A C-on-A
)

# A single lane directly at the base of the target branch (origin/main)
git clone remote empty-workspace-with-branch-below
(cd empty-workspace-with-branch-below
   git checkout -b unrelated

  create_workspace_commit_once unrelated
)

# There are multiple stacked branches that could lead towards a shared stack.
git clone remote target-ahead-remote-rewritten
(cd target-ahead-remote-rewritten
  git checkout -b origin/main
  git commit -m "target ahead" --allow-empty

  git checkout -b A main
  git commit --allow-empty -m "shared local/remote"

  (git checkout -b new-origin
    # a remote commit that looks like a local commit by message
    git commit --allow-empty -m "shared by name"
    git commit --allow-empty -m "unique remote"
    mv .git/refs/heads/new-origin .git/refs/remotes/origin/A
  )
  git checkout A

  git commit --allow-empty -m "unique local"
  # a local commit that looks like a remote commit by message
  git commit --allow-empty -m "shared by name"
  git commit --allow-empty -m "unique local tip"

  create_workspace_commit_once A
)