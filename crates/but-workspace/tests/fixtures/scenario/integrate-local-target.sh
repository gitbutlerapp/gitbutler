#!/usr/bin/env bash
set -eu -o pipefail

git init -b main
git config user.name Test
git config user.email test@example.com
git config commit.gpgsign false
export GIT_AUTHOR_DATE='2000-01-01T00:00:00 +0000'
export GIT_COMMITTER_DATE="$GIT_AUTHOR_DATE"
echo base >file
git add file
git commit -m base
git tag base

git checkout -b upstream-work
echo upstream >>file
git commit -am upstream
git update-ref refs/remotes/origin/main HEAD
git remote add origin https://example.com/upstream.git
git config branch.main.remote origin
git config branch.main.merge refs/heads/main

workspace=$(git commit-tree main^{tree} -p main -m 'GitButler Workspace Commit')
git checkout -b gitbutler/workspace "$workspace"
git branch -D upstream-work
