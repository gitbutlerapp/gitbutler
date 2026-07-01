#!/usr/bin/env bash
set -eu -o pipefail

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

function commit_exact () {
  local message=${1:?}
  git add -A
  local tree
  tree=$(git write-tree)
  local parent_args=()
  if git rev-parse --verify HEAD >/dev/null 2>&1; then
    parent_args=(-p HEAD)
  fi
  local commit
  commit=$(printf "%s" "$message" | git commit-tree "$tree" "${parent_args[@]}")
  local current_branch
  current_branch=$(git symbolic-ref -q HEAD || true)
  if [[ -n "$current_branch" ]]; then
    git update-ref "$current_branch" "$commit"
  fi
  git reset --hard "$commit" >/dev/null
}

function commit_with_tick () {
  local message=${1:?}
  tick
  commit_exact "$message"
}

git init --initial-branch=main remote
(cd remote
  git config user.name "Author"
  git config user.email "author@example.com"
  echo a > file
  git add . && git commit -m "init"
)

git clone remote multiple-commits
(cd multiple-commits
  git config user.name "Author"
  git config user.email "author@example.com"

  git branch existing-branch
  git checkout -b other_stack
  echo change0 >> other_file
  commit_with_tick "commit 0"

  git checkout main
  git checkout -b my_stack
  echo change1 >> file
  commit_with_tick "commit 1"
  echo change2 >> file
  commit_with_tick "commit 2"
  echo change3 >> file
  commit_with_tick "commit 3"

  git branch top-series HEAD
  git checkout top-series
  echo change4 >> file
  commit_with_tick "commit 4"
  echo change5 >> file
  commit_with_tick "commit 5"
  echo change6 >> file
  commit_with_tick "commit 6"

  git checkout -b gitbutler/workspace
  git merge --no-ff -m "GitButler Workspace Commit" other_stack
)

git clone remote multiple-commits-small
(cd multiple-commits-small
  git config user.name "Author"
  git config user.email "author@example.com"

  git branch existing-branch
  git checkout -b other_stack
  echo change0 >> other_file
  commit_with_tick "commit 0"

  git checkout main
  git checkout -b my_stack
  echo change1 >> file
  commit_with_tick "commit 1"

  git branch top-series HEAD
  git checkout top-series
  echo change4 >> file
  commit_with_tick "commit 2"

  git checkout -b gitbutler/workspace
  git merge --no-ff -m "GitButler Workspace Commit" other_stack
)

git clone remote multiple-commits-empty-top
(cd multiple-commits-empty-top
  git config user.name "Author"
  git config user.email "author@example.com"

  git branch existing-branch
  git checkout -b other_stack
  echo change0 >> other_file
  commit_with_tick "commit 0"

  git checkout main
  git checkout -b my_stack
  echo change1 >> file
  commit_with_tick "commit 1"

  git branch top-series HEAD

  git checkout -b gitbutler/workspace
  git merge --no-ff -m "GitButler Workspace Commit" other_stack
)

git clone remote overlapping-commits
tick
(cd overlapping-commits
  git config user.name "Author"
  git config user.email "author@example.com"
  
  git branch existing-branch
  git checkout -b other_stack
  echo change0 >> other_file
  commit_with_tick "commit 0"

  git checkout main
  git checkout -b my_stack
  echo x > file
  commit_with_tick "commit 1"
  echo y > file
  commit_with_tick "commit 2"

  git branch top-series HEAD

  git checkout -b gitbutler/workspace
  git merge --no-ff -m "GitButler Workspace Commit" other_stack
)
