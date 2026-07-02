#!/bin/bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

git init

git commit --allow-empty -m "base"
base=$(git rev-parse HEAD)

# Two parallel (unnamed) commits off base, merged together by M. `feature`
# points at the merge, so its exclusive region is a non-linear diamond
# (M -> {P, Q} -> base).
p=$(git commit-tree "HEAD^{tree}" -p "$base" -m "P")
q=$(git commit-tree "HEAD^{tree}" -p "$base" -m "Q")
m=$(git commit-tree "HEAD^{tree}" -p "$p" -p "$q" -m "M")
git branch feature "$m"

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

# Single-stack workspace commit on top of `feature`.
ws=$(git commit-tree "$m^{tree}" -p feature -m "GitButler Workspace Commit")
git checkout -b gitbutler/workspace
git reset --hard "$ws"
