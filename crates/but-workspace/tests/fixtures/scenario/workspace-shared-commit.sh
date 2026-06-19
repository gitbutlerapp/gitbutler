#!/bin/bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

git init

git commit --allow-empty -m "base"
base=$(git rev-parse HEAD)

# A shared (unnamed) commit S above base, from which two stacks fork. S is
# reachable from both stacks but carries no reference of its own, so it is
# contested between stack-x and stack-y.
s=$(git commit-tree "HEAD^{tree}" -p "$base" -m "S")
x1=$(git commit-tree "HEAD^{tree}" -p "$s" -m "X1")
y1=$(git commit-tree "HEAD^{tree}" -p "$s" -m "Y1")
git branch stack-x "$x1"
git branch stack-y "$y1"

# Setup remote tracking for main (the target lives at base)
mkdir -p .git/refs/remotes/origin
cp .git/refs/heads/main .git/refs/remotes/origin/main

cat <<EOF >>.git/config
[remote "origin"]
	url = ./fake/local/path/which-is-fine-as-we-dont-fetch-or-push
	fetch = +refs/heads/*:refs/remotes/origin/*

[branch "main"]
  remote = "origin"
  merge = refs/heads/main
EOF

# Workspace commit merges the two stack tips.
ws=$(git commit-tree "$x1^{tree}" -p stack-x -p stack-y -m "GitButler Workspace Commit")
git checkout -b gitbutler/workspace
git reset --hard "$ws"
