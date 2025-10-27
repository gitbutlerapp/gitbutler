
# Just to save a line in the scripts that use this module
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

function turn_into_remote_branch() {
  setup_remote_tracking "$@" "move"
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

function tick_committer () {
  if test -z "${tick+set}"; then
    tick=1675176957
  else
    tick=$(($tick + 60))
  fi
  GIT_COMMITTER_DATE="$tick +0100"
  export GIT_COMMITTER_DATE
}

function setup_target_to_match_main() {
  remote_tracking_caught_up main
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

function add_main_remote_setup() {
  cat <<EOF >>.git/config
[remote "origin"]
	url = ./fake/local/path/which-is-fine-as-we-dont-fetch-or-push
	fetch = +refs/heads/*:refs/remotes/origin/*

[branch "main"]
  remote = "origin"
  merge = refs/heads/main
EOF
}

function commit() {
  local message=${1:?first argument is the commit message}
  git commit -am "$message" --allow-empty
}

function commit-file() {
  local name="${1:?First argument is the filename}"
  echo $name >$name && git add $name && git commit -m "add $name"
}

function add_change_id_to_given_commit() {
  local a="00000000-0000-0000-0000-000000000000"
  local b="${1:?first argument is the single-digit number of the change-id}"
  local change_id="${a:0:${#a}-${#b}}${b}"

   # Insert the Change-ID header lines after the committer line.
   git cat-file -p "${2:?second argument is the commit to add a changeid to}" \
   | awk -v cid="$change_id" '
     BEGIN { injected = 0 }
     /^$/ && !injected {
       print "gitbutler-headers-version 2"
       print "gitbutler-change-id " cid
       print ""
       injected = 1
       next
     }
     { print }
     ' \
   | git hash-object -wt commit --stdin
}

function write_ref_safely() {
  cat >.git/tmp
  mv .git/tmp "${1:?first argument is the full path to the ref to write}"
}