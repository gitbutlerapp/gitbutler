#!/bin/bash

set -eu -o pipefail

source "$(dirname "$0")/shared.sh"

git init
git commit --allow-empty -m "base"
git tag base

printf 'local work\n' >local-only
git add local-only
git commit -m "unpublished local work"

git checkout -b landed base
printf 'upstream work\n' >upstream-only
git add upstream-only
git commit -m "landed upstream work"

git update-ref refs/remotes/origin/main landed
git config remote.origin.url ./unused-remote
git config remote.origin.fetch '+refs/heads/*:refs/remotes/origin/*'
git config branch.main.remote origin
git config branch.main.merge refs/heads/main

git checkout -b gitbutler/workspace main
git merge --no-ff landed -m "GitButler Workspace Commit"
