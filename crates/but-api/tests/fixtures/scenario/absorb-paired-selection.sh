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

for line in {1..10}; do
    printf 'old-%02d\n' "$line"
done >selected.txt
git add selected.txt
git commit -m "add selected lines"

for line in {1..10}; do
    printf 'new-%02d\n' "$line"
done >selected.txt