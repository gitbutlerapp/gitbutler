#!/usr/bin/env bash

### General Description

# Various directories with different scenarios for testing stack information *with* or *without* a workspace commit.
set -eu -o pipefail

function remote_tracking_caught_up() {
  setup_remote_tracking "$1"
}

function setup_remote_tracking() {
  local branch_name="${1:?}"
  local remote_branch_name=${2:-"$branch_name"}
  local mode=${3:-"cp"}
  mkdir -p .git/refs/remotes/origin

  if [[ "$mode" == "cp" ]]; then
    cp ".git/refs/heads/$branch_name" ".git/refs/remotes/origin/$remote_branch_name"
  else
    mv ".git/refs/heads/$branch_name" ".git/refs/remotes/origin/$remote_branch_name"
  fi
}

function tick () {
  if test -z "${tick+set}"; then
    tick=1675176957
  else
    tick=$(($tick + 60))
  fi
  GIT_COMMITTER_DATE="$tick +0100"
  GIT_AUTHOR_DATE="$tick +0100"
  export GIT_COMMITTER_DATE GIT_AUTHOR_DATE
}

function setup_target_to_match_main() {
  remote_tracking_caught_up main

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

function commit() {
  local message=${1:?first argument is the commit message}
  git commit -m "$message" --allow-empty
}

git init unborn
git init detached
(cd detached
  commit init && git branch other
  commit first && git tag release/v1 && git tag -am "tag object" annotated
)

# A top-down split that is highly unusual, but good to assure we can handle it.
git init multi-root
(cd multi-root
  commit A
  git checkout --orphan B && commit B
  git checkout --orphan C && commit C
  git checkout --orphan D && commit D

  git checkout main && git merge --allow-unrelated-histories B
  git checkout C && git merge --allow-unrelated-histories D

  git checkout main && git merge --allow-unrelated-histories C
)

# A single root that splits up into 4 branches and merges again
git init four-diamond
(cd four-diamond
  commit base
  git checkout -b A && commit A
  git checkout -b B main && commit B
  git checkout -b C main && commit C
  git checkout -b D main && commit D

  git checkout A && git merge B
  git checkout C && git merge D

  git checkout -B merged A && git merge C
)

mkdir ws
(cd ws
  git init single-stack-ambiguous
  (cd single-stack-ambiguous
     commit init
       setup_target_to_match_main
       git branch new-A
       git branch new-B
     git checkout -b A
       commit segment-A
       for name in A-empty-01 A-empty-02 A-empty-03; do
         git branch "$name"
       done
     git checkout -b B
       commit segment-B~1 && git branch B-empty && git branch ambiguous-01
       commit segment-B && git branch ambiguous-02
     create_workspace_commit_once B
  )

  git init single-stack
  (cd single-stack
     commit init
       setup_target_to_match_main
       git branch new-A
     git checkout -b A
       commit segment-A
     git checkout -b B
       commit segment-B~1
         git branch B-sub
       commit segment-B
     create_workspace_commit_once B
  )

  git init dual-merge
  (cd dual-merge
     commit init
       setup_target_to_match_main
       git branch B
     git checkout -b A
       commit A
     git checkout B
       commit B
     git checkout -b merge
       git merge --no-ff A
       git branch empty-1-on-merge
       git branch empty-2-on-merge
     git checkout -b C
       git branch D
       commit C
     git checkout D
       commit D
     git checkout -b merge-2
       git merge --no-ff C
     create_workspace_commit_once merge-2
  )

  git init just-init-with-branches
  (cd just-init-with-branches
    commit init && setup_target_to_match_main
    for name in A B C D E F gitbutler/workspace; do
      git branch "$name"
    done
  )
)
