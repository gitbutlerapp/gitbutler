#!/usr/bin/env bash
set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

init-repo-with-files-and-remote

# Scenario 4: A workspace with no applied stacks

# Create an unapplied branch with a commit to test cherry-picking
git checkout -b no-stacks-branch main
echo "some change" > shared.txt
git add . && git commit -m "Change to shared.txt"
git tag no-stacks-commit

# Store this as a GitButler reference
git update-ref refs/gitbutler/no-stacks-commit no-stacks-commit

git checkout -b gitbutler/workspace main
