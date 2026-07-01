#!/usr/bin/env bash
set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

init-repo-with-files-and-remote

# Create feature-a stack (base branch)
git checkout -b feature-a
echo "feature-a line 1" > foo.txt
git add foo.txt && commit "feature-a: add line 1"
echo "feature-a line 2" > bar.txt
git add bar.txt && commit "feature-a: add line 2"

# Create feature-b stack (stacked on feature-a)
git checkout -b feature-b
echo "feature-b line 1" > foo.txt
commit "feature-b: add line 1"
echo "feature-b line 2" > bar.txt
commit "feature-b: add line 2"

create_workspace_commit_once feature-b
