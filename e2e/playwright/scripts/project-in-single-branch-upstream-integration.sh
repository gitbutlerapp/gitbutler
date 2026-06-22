#!/bin/bash

set -euo pipefail

echo "GIT CONFIG $GIT_CONFIG_GLOBAL"
echo "DATA DIR $E2E_TEST_APP_DATA_DIR"
echo "BUT $BUT"
echo "SCENARIO: $1"

scenario="$1"

mkdir remote-project
pushd remote-project
git init -b master --object-format=sha1
echo "base line 1" >> a_file
echo "base line 2" >> a_file
echo "base line 3" >> a_file
git add a_file
git commit -m "base: initial commit"

case "$scenario" in
  fully-integrated)
    git checkout -b fully-integrated-branch
    echo "fully integrated commit 1" > fully_integrated_first.txt
    git add fully_integrated_first.txt
    git commit -m "fully-integrated: first commit"
    echo "fully integrated commit 2" > fully_integrated_second.txt
    git add fully_integrated_second.txt
    git commit -m "fully-integrated: second commit"
    git checkout master
    ;;
  partial-stack)
    git checkout -b partial-stack-base
    echo "partial stack base" > partial_stack_base.txt
    git add partial_stack_base.txt
    git commit -m "partial-stack-base: first commit"

    git checkout -b partial-stack-top
    echo "partial stack top" > partial_stack_top.txt
    git add partial_stack_top.txt
    git commit -m "partial-stack-top: first commit"
    git checkout master
    ;;
  rebase)
    ;;
  *)
    echo "Unknown scenario: $scenario" >&2
    exit 1
    ;;
esac
popd

git clone remote-project local-clone
pushd local-clone
git checkout master

if [ "$scenario" = "rebase" ]; then
  git checkout -b rebased-single-branch master
  echo "rebased branch commit 1" > rebased_single_first.txt
  git add rebased_single_first.txt
  git commit -m "rebased-single-branch: first commit"
  echo "rebased branch commit 2" > rebased_single_second.txt
  git add rebased_single_second.txt
  git commit -m "rebased-single-branch: second commit"
  git checkout master
fi

"$BUT" setup

case "$scenario" in
  fully-integrated)
    "$BUT" apply fully-integrated-branch
    git checkout fully-integrated-branch
    ;;
  partial-stack)
    "$BUT" apply partial-stack-base
    "$BUT" apply partial-stack-top
    git checkout partial-stack-top
    ;;
  rebase)
    "$BUT" apply rebased-single-branch
    git checkout rebased-single-branch
    ;;
esac
popd
