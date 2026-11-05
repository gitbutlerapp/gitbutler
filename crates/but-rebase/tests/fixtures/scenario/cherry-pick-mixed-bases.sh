#!/bin/bash

# Workspace commits whose parents rest on different commits of the target's history.
#
# base ─── main ─┬─ reverts-main  `file` is `one` again
#   │            ├─ edits-main    `file` is `four`
#   │            └─ adds-main     adds `on-main`
#   ├─ older                      `file` is `three`
#   └─ adds-base                  adds `on-base`
#
# `file` is `one` in `base` and `two` in `main`.

set -eu -o pipefail

git init

echo one > file && git add . && git commit -m "base"
git branch base
echo two > file && git commit -am "main"

git switch -c reverts-main main
echo one > file && git commit -am "reverts-main"

git switch -c edits-main main
echo four > file && git commit -am "edits-main"

git switch -c adds-main main
echo main > on-main && git add . && git commit -m "adds-main"

git switch -c older base
echo three > file && git commit -am "older"

git switch -c adds-base base
echo base > on-base && git add . && git commit -m "adds-base"

# Git decides the expected tree of each workspace.
git switch -c ws-older-reverts-main older
git merge --no-ff -m "ws-older-reverts-main" reverts-main

git switch -c ws-adds-base-reverts-main adds-base
git merge --no-ff -m "ws-adds-base-reverts-main" reverts-main

git switch -c ws-edits-main-adds-main edits-main
git merge --no-ff -m "ws-edits-main-adds-main" adds-main

git switch -c ws-adds-base-edits-main adds-base
git merge --no-ff -m "ws-adds-base-edits-main" edits-main

git switch -c ws-adds-base-edits-main-adds-main
git merge --no-ff -m "ws-adds-base-edits-main-adds-main" adds-main
