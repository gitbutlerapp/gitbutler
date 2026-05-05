#!/bin/bash

echo "GIT CONFIG $GIT_CONFIG_GLOBAL"
echo "DATA DIR $E2E_TEST_APP_DATA_DIR"
echo "BUT $BUT"

# Setup a remote project with two branches that fork from DIFFERENT master
# commits. This causes stacks to have different merge bases with the target,
# which triggers IncompatibleBase rejections when committing a shared file
# to the wrong stack.
#
#   master:  init ──── upstream-change
#              │              │
#          old-stack      new-stack
#      (doesn't have     (has the upstream
#       upstream change)  change)

mkdir remote-project
pushd remote-project
git init -b master --object-format=sha1

# Initial commit with a shared file
cat > shared_file << 'CONTENT'
alpha
bravo
charlie
delta
echo
foxtrot
golf
hotel
india
juliet
CONTENT
git add shared_file
git commit -m "Initial commit with shared_file"

# Create old-stack branch from here (before the upstream change)
git checkout -b old-stack
echo "old-stack work" >> other_file
git add other_file
git commit -m "old-stack: add other_file"
git checkout master

# Now advance master with a change to shared_file
sed 's/juliet/JULIET-UPSTREAM/' shared_file > shared_file.tmp && mv shared_file.tmp shared_file
git commit -am "Upstream change to shared_file"

# Create new-stack branch from updated master (has the upstream change)
git checkout -b new-stack
echo "new-stack work" >> another_file
git add another_file
git commit -m "new-stack: add another_file"
git checkout master
popd

# Clone the remote and set up GitButler
git clone remote-project local-clone
pushd local-clone
  git checkout master
  target_branch="$(git rev-parse --symbolic-full-name @{u})"
  target_branch="${target_branch#refs/remotes/}"
  "$BUT" setup
  "$BUT" config target "$target_branch"
popd
