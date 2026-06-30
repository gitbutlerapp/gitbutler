#!/bin/bash

set -euo pipefail

echo "GIT CONFIG $GIT_CONFIG_GLOBAL"
echo "DATA DIR $E2E_TEST_APP_DATA_DIR"
echo "BUT $BUT"

# Start with a remote-backed project because GitButler setup expects one in e2e.
mkdir remote-project
pushd remote-project
git init -b master --object-format=sha1
echo "base" > base.txt
git add base.txt
git commit -m "base: initial commit"

git checkout -b stale-workspace-branch
echo "stale workspace branch" > stale_workspace_branch.txt
git add stale_workspace_branch.txt
git commit -m "stale-workspace-branch: first commit"

git checkout master
git checkout -b single-branch-fixture
echo "single branch" > single_branch_file.txt
git add single_branch_file.txt
git commit -m "single-branch: first commit"

git checkout master
git checkout -b branch-to-apply
echo "branch to apply" > branch_to_apply.txt
git add branch_to_apply.txt
git commit -m "branch-to-apply: first commit"

git checkout master
popd

git clone remote-project local-clone
pushd local-clone
git checkout master
target_branch="$(git rev-parse --symbolic-full-name @{u})"
target_branch="${target_branch#refs/remotes/}"
"$BUT" setup
"$BUT" config target "$target_branch"

# Create a managed workspace whose metadata will become stale after the direct checkout below.
"$BUT" apply stale-workspace-branch

git checkout -b single-branch-fixture origin/single-branch-fixture
popd
