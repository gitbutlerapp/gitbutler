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
git remote add origin https://example.com/upstream.git
git update-ref refs/remotes/origin/main HEAD
git config branch.main.remote origin
git config branch.main.merge refs/heads/main
echo local >>file
git commit -am local
git checkout -b feature
echo feature >>file
git commit -am feature
