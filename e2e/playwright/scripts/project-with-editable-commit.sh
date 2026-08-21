#!/bin/bash

echo "GIT CONFIG $GIT_CONFIG_GLOBAL"
echo "DATA DIR $E2E_TEST_APP_DATA_DIR"
echo "BUT $BUT"

# A project holding one local commit that edit mode can check out. The file has
# enough surrounding context that a one-line change stays a single hunk.

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
  "$BUT" commit -b edit-branch -m "Change juliet locally"
popd
