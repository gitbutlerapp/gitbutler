#!/usr/bin/env bash

set -eu -o pipefail

git init -b main
git config user.name GitButler
git config user.email gitbutler@example.com
export GIT_AUTHOR_DATE="2000-01-01 00:00:00 +0000"
export GIT_COMMITTER_DATE="2000-01-02 00:00:00 +0000"

git commit --allow-empty -m "base"
git update-ref refs/remotes/origin/main HEAD

git checkout -b A
for line in {1..20}; do
    printf 'A line %s\n' "$line"
done >a.txt
git add a.txt
git commit -m "A change"

git checkout -b B main
for line in {1..20}; do
    printf 'B line %s\n' "$line"
done >b.txt
git add b.txt
git commit -m "B change"

git checkout -b gitbutler/workspace main
git merge --no-ff -m "GitButler Workspace Commit" A B

for line in {1..20}; do
    if [[ "$line" == 10 ]]; then
        printf 'A selected change\n'
    else
        printf 'A line %s\n' "$line"
    fi
done >a.txt

for line in {1..20}; do
    if [[ "$line" == 10 ]]; then
        printf 'B selected change\n'
    else
        printf 'B line %s\n' "$line"
    fi
done >b.txt