#!/usr/bin/env bash
set -eu -o pipefail

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

git init --initial-branch=main remote
(cd remote
  git config user.name "Author"
  git config user.email "author@example.com"
  echo a > file
  git add . && git commit -m "init"
)

# Scenario:
# - commit 5 (a-branch-3)
# - commit 4 (a-branch-2)
# - commit 3
# - commit 2
# - commit 1 (a-branch-1)
git clone remote multiple-commits
(cd multiple-commits
  git config user.name "Author"
  git config user.email "author@example.com"

  git branch existing-branch
  git checkout --detach main
  echo change1 >> file1
  commit_exact "commit 1"
  commit_1=$(git rev-parse HEAD)

  echo change2 > file2_3
  commit_exact "commit 2"
  echo change3 > file2_3
  commit_exact "commit 3"

  echo change4 > file4
  commit_exact "commit 4"
  commit_4=$(git rev-parse HEAD)

  echo change5 >> file5
  commit_exact "commit 5"
  commit_5=$(git rev-parse HEAD)

  git branch my_stack "$commit_1"
  git branch a-branch-2 "$commit_4"
  git branch a-branch-3 "$commit_5"
  git checkout -b gitbutler/workspace "$commit_5"
  git commit --allow-empty -m "GitButler Workspace Commit"
)
