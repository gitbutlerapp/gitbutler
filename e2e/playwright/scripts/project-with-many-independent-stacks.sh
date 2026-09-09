#!/bin/bash

set -euo pipefail

echo "GIT CONFIG $GIT_CONFIG_GLOBAL"
echo "DATA DIR $E2E_TEST_APP_DATA_DIR"
echo "BUT $BUT"

stack_count="${1:-12}"

# Setup a remote project. GitButler currently requires projects to have a remote.
mkdir remote-project
pushd remote-project
git init -b master --object-format=sha1
echo "base" > base.txt
git add base.txt
git commit -m "base: initial commit"
popd

# Clone the remote, register the project with GitButler, configure the target,
# then fill the workspace with enough independent stacks that the list of them
# is taller than the panel showing it and has to scroll. Committing to a branch
# that does not exist yet creates it unstacked, so each one is its own stack.
git clone remote-project local-clone
pushd local-clone
git checkout master
target_branch="$(git rev-parse --symbolic-full-name @{u})"
target_branch="${target_branch#refs/remotes/}"
"$BUT" setup
"$BUT" config target "$target_branch"

for index in $(seq 1 "$stack_count"); do
  echo "stack $index" > "stack-$index.txt"
  "$BUT" commit -m "stack-$index: first commit" --branch "stack-$index"
  echo "stack $index again" >> "stack-$index.txt"
  "$BUT" commit -m "stack-$index: second commit" --branch "stack-$index"
done
popd
