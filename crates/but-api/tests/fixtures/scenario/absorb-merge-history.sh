#!/usr/bin/env bash

set -eu -o pipefail

git init -b main
git config user.name GitButler
git config user.email gitbutler@example.com
export GIT_AUTHOR_DATE="2000-01-01 00:00:00 +0000"
export GIT_COMMITTER_DATE="2000-01-02 00:00:00 +0000"

git commit --allow-empty -m "base"
git update-ref refs/remotes/origin/main HEAD

git checkout -b feature
printf 'feature old\n' >feature.txt
git add feature.txt
git commit -m "feature content"

git checkout -b side main
printf 'side content\n' >side.txt
git add side.txt
git commit -m "side content"

git checkout feature
git merge --no-ff -m "merge side" side
printf 'feature new\n' >feature.txt