#!/usr/bin/env bash

set -eu -o pipefail
source "${BASH_SOURCE[0]%/*}/shared.sh"

### Description
# Heads on different commits of the target's history. `file` is `one` in `base` and `two` in `main`.
#
# base ─── main ─┬─ edits-main    `file` is `four`
#   │            ├─ reverts-main  `file` is `one` again
#   │            └─ adds-main     adds `on-main`
#   ├─ adds-base                  adds `on-base`
#   └─ adds-other                 adds `other`
#
# Each `ws/<parents>` is a workspace commit of these parents, with the tree Git merges them to.
function workspace_commit() {
  local parents=()
  git checkout -q --detach "$1"
  for parent in "$@"; do
    parents+=(-p "$parent")
    git merge -q --no-ff -m "merge $parent" "$parent"
  done
  local IFS=+
  git branch "ws/$*" "$(git commit-tree -m "GitButler Workspace Commit" "${parents[@]}" "HEAD^{tree}")"
}

git init
echo one >file && git add file && git commit -m "base"
git checkout -b adds-base
echo base >on-base && git add on-base && git commit -m "adds-base"
git checkout -b adds-other main
echo other >other && git add other && git commit -m "adds-other"

git checkout main
echo two >file && git commit -am "main"
setup_target_to_match_main
git branch empty-at-main

git checkout -b edits-main
echo four >file && git commit -am "edits-main"
git checkout -b reverts-main main
echo one >file && git commit -am "reverts-main"
git checkout -b adds-main main
echo main >on-main && git add on-main && git commit -m "adds-main"

workspace_commit adds-base edits-main
workspace_commit adds-base edits-main empty-at-main
workspace_commit empty-at-main adds-base edits-main
workspace_commit adds-base reverts-main
workspace_commit empty-at-main adds-base
workspace_commit adds-base reverts-main empty-at-main

git checkout -b gitbutler/workspace main
