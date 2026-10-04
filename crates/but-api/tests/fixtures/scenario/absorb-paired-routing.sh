#!/usr/bin/env bash

set -eu -o pipefail

git init -b main
git config user.name GitButler
git config user.email gitbutler@example.com
export GIT_AUTHOR_DATE="2000-01-01 00:00:00 +0000"
export GIT_COMMITTER_DATE="2000-01-02 00:00:00 +0000"

for line in {1..24}; do
    printf 'base-%02d\n' "$line"
done >routed.txt
git add routed.txt
git commit -m "base"
git update-ref refs/remotes/origin/main HEAD
git checkout -b feature

for line in {1..24}; do
    case "$line" in
    5|6) printf 'first-%02d\n' "$line" ;;
    *) printf 'base-%02d\n' "$line" ;;
    esac
done >routed.txt
git add routed.txt
git commit -m "change first region"

for line in {1..24}; do
    case "$line" in
    5|6) printf 'first-%02d\n' "$line" ;;
    15|16) printf 'second-%02d\n' "$line" ;;
    *) printf 'base-%02d\n' "$line" ;;
    esac
done >routed.txt
git add routed.txt
git commit -m "change second region"

for line in {1..24}; do
    case "$line" in
    5|6) printf 'work-%02d\n' "$line" ;;
    15|16) printf 'second-%02d\n' "$line" ;;
    *) printf 'base-%02d\n' "$line" ;;
    esac
done >routed.txt