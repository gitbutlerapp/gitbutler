#!/usr/bin/env bash
set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

git init -b main
tick
commit base
git tag base
git remote add origin https://example.com/upstream.git
git update-ref refs/remotes/origin/main HEAD
git config branch.main.remote origin
git config branch.main.merge refs/heads/main

echo local >local-only
git add local-only
git commit -m local-only

git checkout -b inactive
echo feature >feature-only
git add feature-only
git commit -m feature-only

workspace=$(git commit-tree base^{tree} -p base -m 'GitButler Workspace Commit')
git checkout -b gitbutler/workspace "$workspace"
