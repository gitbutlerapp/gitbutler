#!/bin/bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

git init
echo "Two applied stacks, bar-one and bar-two, that share the underlying branch 'foo'" >.git/description

git commit --allow-empty -m "M1"

# The shared branch both stacks are built on.
git checkout -b foo
git commit --allow-empty -m "F1"

git checkout -b bar-one foo
git commit --allow-empty -m "B1"

git checkout -b bar-two foo
git commit --allow-empty -m "B2"

remote_tracking_caught_up main
add_main_remote_setup

# Create workspace commit merging both stack tips.
id=$(git commit-tree HEAD^{tree} -p bar-one -p bar-two -m "GitButler Workspace Commit")
git checkout -b gitbutler/workspace
git reset --hard $id
