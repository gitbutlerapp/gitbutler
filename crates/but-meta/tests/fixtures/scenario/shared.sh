#!/usr/bin/env bash

function commit() {
  local message=${1:?first argument is the commit message}
  git commit -am "$message" --allow-empty
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

function setup_target_to_match_main() {
  setup_remote_tracking main
  # This is for `but` which needs it right now to set a target.
  echo "ref: refs/remotes/origin/main" >.git/refs/remotes/origin/HEAD

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
