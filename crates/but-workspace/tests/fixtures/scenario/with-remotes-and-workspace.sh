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

function setup_target_to_match_main() {
  mkdir -p .git/refs/remotes/origin
  cp .git/refs/heads/main .git/refs/remotes/origin/

  cat <<EOF >>.git/config
[remote "origin"]
	url = ./fake/local/path/which-is-fine-as-we-dont-fetch-or-push
	fetch = +refs/heads/*:refs/remotes/origin/*
EOF
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
    git merge --no-ff -m "$workspace_commit_subject" "${@}"
  fi
}

# can only be called once per test setup, and definitely doesn't do anything smart like the above version.
# TODO: Both yield different results due to the way the merge is done, so that's maybe something to double-check as well.
function create_workspace_commit_aggressively() {
  local workspace_commit_subject="GitButler Workspace Commit"

  if [ $# == 1 ]; then
    local current_branch=$(git rev-parse --abbrev-ref HEAD)
    if [[ "$current_branch" != "$1" ]]; then
      echo "BUG: Must assure the current branch is the branch passed as argument: $current_branch != $1"
      return 42
    fi
  fi

  git checkout -b gitbutler/workspace main
  if [ $# == 1 ] || [ $# == 0 ]; then
    git commit --allow-empty -m "$workspace_commit_subject"
  else
    git merge --no-ff --strategy octopus -m "$workspace_commit_subject" "${@}"
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

git init disjoint
(cd disjoint
  git commit -m "init" --allow-empty
  setup_target_to_match_main

  git checkout --orphan disjoint
  git commit -m "disjoint init" --allow-empty
)

git init two-branches-one-advanced-one-parent-ws-commit
(cd two-branches-one-advanced-one-parent-ws-commit
  git commit -m "init" --allow-empty
  setup_target_to_match_main
  git checkout -b lane main

  git branch advanced-lane-2
  git checkout -b advanced-lane
  git commit -m "change" --allow-empty

  git checkout advanced-lane-2
  git commit -m "change 2" --allow-empty

  create_workspace_commit_once advanced-lane-2 advanced-lane
)

# TTB = target-tracking-branch
git init two-branches-one-advanced-two-parent-ws-commit-diverged-ttb
(cd two-branches-one-advanced-two-parent-ws-commit-diverged-ttb
  git commit -m "init" --allow-empty
  git checkout -b lane main

  git checkout -b advanced-lane
  git commit -m "change" --allow-empty

  create_workspace_commit_aggressively advanced-lane lane
  # swap trees - Git puts 'lane' first for some reason, but we really need the other way to reproduce a bug!
  commit_swapped_parents=$(git commit-tree -p "HEAD^2" -p "HEAD^1" -m "GitButler Workspace Commit" "HEAD^{tree}")
  echo "${commit_swapped_parents}" >.git/refs/heads/gitbutler/workspace

  git checkout --orphan disjoint-target-tracking
  git commit -m "disjoint remote target" --allow-empty

  mkdir -p .git/refs/remotes/origin
  mv .git/refs/heads/disjoint-target-tracking .git/refs/remotes/origin/main

  git checkout gitbutler/workspace
)

git init two-branches-one-advanced-two-parent-ws-commit
(cd two-branches-one-advanced-two-parent-ws-commit
  git commit -m "init" --allow-empty
  setup_target_to_match_main
  git checkout -b lane main

  git checkout -b advanced-lane
  git commit -m "change" --allow-empty

  create_workspace_commit_aggressively lane advanced-lane
)

cp -R two-branches-one-advanced-two-parent-ws-commit two-branches-one-advanced-two-parent-ws-commit-advanced-fully-pushed
(cd two-branches-one-advanced-two-parent-ws-commit-advanced-fully-pushed
  # This works without an official remote setup as we go by name as fallback.
  cp .git/refs/heads/advanced-lane .git/refs/remotes/origin/advanced-lane
)

cp -R two-branches-one-advanced-two-parent-ws-commit-advanced-fully-pushed two-branches-one-advanced-two-parent-ws-commit-advanced-fully-pushed-empty-dependant
(cd two-branches-one-advanced-two-parent-ws-commit-advanced-fully-pushed-empty-dependant
  git branch dependant advanced-lane
)

git init three-branches-one-advanced-ws-commit-advanced-fully-pushed-empty-dependant
(cd three-branches-one-advanced-ws-commit-advanced-fully-pushed-empty-dependant
  git commit -m "init" --allow-empty
  setup_target_to_match_main
  git checkout -b lane main

  git checkout -b advanced-lane
  git commit -m "change" --allow-empty
  # This works without an official remote setup as we go by name as fallback.
  cp .git/refs/heads/advanced-lane .git/refs/remotes/origin/advanced-lane
  git branch dependant
  git branch on-top-of-dependant

  create_workspace_commit_once advanced-lane
)

git init two-branches-one-advanced-ws-commit-on-top-of-stack
(cd two-branches-one-advanced-ws-commit-on-top-of-stack
  git commit -m "init" --allow-empty
  setup_target_to_match_main
  git checkout -b lane main

  git checkout -b advanced-lane
  git commit -m "change" --allow-empty

  create_workspace_commit_once lane advanced-lane
)
git init multiple-dependent-branches-per-stack-without-commit
(cd multiple-dependent-branches-per-stack-without-commit
  git commit -m "init" --allow-empty
  setup_target_to_match_main

  git branch lane-segment-01
  git branch lane-segment-02

  git branch lane-2
  git branch lane-2-segment-01
  git branch lane-2-segment-02

  git checkout -b lane
  git commit -m "change" --allow-empty

  create_workspace_commit_once lane lane-2
)
