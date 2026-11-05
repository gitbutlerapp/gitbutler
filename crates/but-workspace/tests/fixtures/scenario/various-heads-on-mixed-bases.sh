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
# A workspace ref is present to make it easier to discover the branches of interest in the graph.
git init
echo one >file && git add file && git commit -m "base"
git checkout -b adds-base
echo base >on-base && git add on-base && git commit -m "adds-base"
git checkout -b adds-other main
echo other >other && git add other && git commit -m "adds-other"

git checkout main
echo two >file && git commit -am "main"
git branch gitbutler/workspace
git branch empty-at-main

git checkout -b edits-main
echo four >file && git commit -am "edits-main"
git checkout -b reverts-main main
echo one >file && git commit -am "reverts-main"
git checkout -b adds-main main
echo main >on-main && git add on-main && git commit -m "adds-main"
