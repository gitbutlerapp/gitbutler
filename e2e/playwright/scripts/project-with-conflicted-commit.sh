#!/bin/bash

echo "GIT CONFIG $GIT_CONFIG_GLOBAL"
echo "DATA DIR $E2E_TEST_APP_DATA_DIR"
echo "BUT $BUT"

# A project whose only commit is conflicted: it changes the same line the base
# moved underneath it, so rebasing onto the new base cannot apply it. The
# commit's files carry conflict markers once edit mode checks them out.

mkdir remote-project
pushd remote-project
git init -b master --object-format=sha1
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
CONTENT
git add a_file
git commit -m "Initial commit with phonetic alphabet"
popd

git clone remote-project local-clone
pushd local-clone
  git checkout master
  target_branch="$(git rev-parse --symbolic-full-name @{u})"
  target_branch="${target_branch#refs/remotes/}"
  "$BUT" setup
  "$BUT" config target "$target_branch"

  sed 's/^juliet$/JULIET-LOCAL/' a_file > a_file.tmp && mv a_file.tmp a_file
  "$BUT" commit -b conflict-branch -m "Change juliet locally"
popd

# Move the base onto the same line, so the local commit can no longer apply.
pushd remote-project
  sed 's/^juliet$/JULIET-UPSTREAM/' a_file > a_file.tmp && mv a_file.tmp a_file
  git commit -am "Change juliet upstream"
popd

pushd local-clone
  git fetch
  # Rebasing always completes in GitButler; the commit is left marked conflicted.
  "$BUT" pull
popd

# `but pull` reports the conflict through its exit code, which is the expected
# outcome here rather than a fixture failure.
true
