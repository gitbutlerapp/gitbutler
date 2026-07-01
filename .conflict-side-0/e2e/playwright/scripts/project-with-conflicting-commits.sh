#!/bin/bash

echo "GIT CONFIG $GIT_CONFIG_GLOBAL"
echo "DATA DIR $E2E_TEST_APP_DATA_DIR"
echo "BUT $BUT"

# Setup a remote project with a branch that has two commits
# modifying the same lines of the same file. This makes it possible
# to trigger a cherry-pick merge conflict when amending the first commit
# with worktree changes.

mkdir remote-project
pushd remote-project
git init -b master --object-format=sha1

# Base commit with a file that has enough lines for meaningful diffs.
# The extra context lines ensure that hunks are anchored properly
# so that cherry-pick conflicts are detected.
cat > a_file << 'CONTENT'
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
kilo
lima
mike
november
oscar
papa
quebec
romeo
sierra
tango
CONTENT
git add a_file
git commit -m "Initial commit with phonetic alphabet"

# Branch with two commits that both change the SAME line (line 10: "juliet")
git checkout -b conflicting-branch

# Commit 1: change "juliet" to "JULIET-FIRST"
sed 's/juliet/JULIET-FIRST/' a_file > a_file.tmp && mv a_file.tmp a_file
git commit -am "Change juliet to JULIET-FIRST"

# Commit 2: change "JULIET-FIRST" to "JULIET-SECOND"
sed 's/JULIET-FIRST/JULIET-SECOND/' a_file > a_file.tmp && mv a_file.tmp a_file
git commit -am "Change juliet to JULIET-SECOND"

git checkout master
popd

# Clone the remote and add the project
git clone remote-project local-clone
pushd local-clone
  git checkout master
  target_branch="$(git rev-parse --symbolic-full-name @{u})"
  target_branch="${target_branch#refs/remotes/}"
  "$BUT" setup
  "$BUT" config target "$target_branch"
popd
