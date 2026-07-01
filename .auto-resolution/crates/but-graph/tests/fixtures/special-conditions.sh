#!/usr/bin/env bash

source "${BASH_SOURCE[0]%/*}/shared.sh"

git init shallow-clone-depth-2-source
(cd shallow-clone-depth-2-source
  for idx in $(seq 4); do
    commit "commit $idx"
  done
)
git clone --depth 2 "file://$PWD/shallow-clone-depth-2-source" shallow-clone-depth-2

git init shallow-workspace-source
(
  cd shallow-workspace-source
  commit M1
  commit M2
  commit M3
  commit M4
  git checkout -b A
  commit A1
  create_workspace_commit_once A
)

git clone --depth 3 --no-single-branch \
  "file://$PWD/shallow-workspace-source" \
  shallow-workspace-boundary-below-lower-bound
(
  cd shallow-workspace-boundary-below-lower-bound
  git branch A origin/A
  git branch main origin/main
)

git clone --depth 2 \
  "file://$PWD/shallow-workspace-source" \
  shallow-workspace-boundary-in-workspace
(
  cd shallow-workspace-boundary-in-workspace
  git branch A HEAD^
)

# This fixture creates a merge where the first parent has an earlier committer
# timestamp than the second parent. The traversal queue processes younger
# commits first, so this captures parent-order handling when the second parent is
# discovered before the first parent.
git init merge-first-parent-older
(
  cd merge-first-parent-older

  GIT_COMMITTER_DATE="2020-01-01T00:00:00Z" GIT_AUTHOR_DATE="2020-01-01T00:00:00Z" \
    git commit --allow-empty -m "base" && git tag base

  git checkout -b first-parent
  GIT_COMMITTER_DATE="2020-01-02T00:00:00Z" GIT_AUTHOR_DATE="2020-01-02T00:00:00Z" \
    git commit --allow-empty -m "old commit on first-parent"

  git checkout -b second-parent main
  GIT_COMMITTER_DATE="2024-06-01T00:00:00Z" GIT_AUTHOR_DATE="2024-06-01T00:00:00Z" \
    git commit --allow-empty -m "new commit 1 on second-parent"
  GIT_COMMITTER_DATE="2024-06-02T00:00:00Z" GIT_AUTHOR_DATE="2024-06-02T00:00:00Z" \
    git commit --allow-empty -m "new commit 2 on second-parent"
  GIT_COMMITTER_DATE="2024-06-03T00:00:00Z" GIT_AUTHOR_DATE="2024-06-03T00:00:00Z" \
    git commit --allow-empty -m "new commit 3 on second-parent"

  git checkout first-parent
  GIT_COMMITTER_DATE="2024-07-01T00:00:00Z" GIT_AUTHOR_DATE="2024-07-01T00:00:00Z" \
    git merge --no-ff second-parent -m "merge second-parent into first-parent"

  GIT_COMMITTER_DATE="2024-07-02T00:00:00Z" GIT_AUTHOR_DATE="2024-07-02T00:00:00Z" \
    git commit --allow-empty -m "commit on top of merge"
)
