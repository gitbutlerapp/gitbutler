#!/usr/bin/env bash
set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

init-repo-with-files-and-remote

# Create feature-a stack (base branch)
git checkout -b feature-a
echo "feature-a line 1" >> file.txt
commit "feature-a: add line 1"
echo "feature-a line 2" >> file.txt
commit "feature-a: add line 2"

# Create feature-b stack (stacked on feature-a)
git checkout -b feature-b
echo "feature-b line 1" >> file.txt
commit "feature-b: add line 1"
echo "feature-b line 2" >> file.txt
commit "feature-b: add line 2"

# Create feature-c stack (parallel to feature-a)
git checkout -b feature-c main
echo "feature-c content" > feature-c.txt
git add feature-c.txt && commit "feature-c: add new file"
echo "feature-c line 2" >> feature-c.txt
commit "feature-c: add line 2"

create_workspace_commit_once feature-c feature-b