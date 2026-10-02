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

for line in {1..20}; do
    printf 'line %s\n' "$line"
done >shared.txt
git add shared.txt
git commit -m "add shared file"

for line in {1..20}; do
    case "$line" in
    1) printf 'unselected change\n' ;;
    10) printf 'selected change\n' ;;
    *) printf 'line %s\n' "$line" ;;
    esac
done >shared.txt