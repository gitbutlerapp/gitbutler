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
  empty-top-over-integrated)
    git checkout -b integrated-branch-under-empty-top
    echo "integrated lower commit 1" > integrated_under_empty_top_first.txt
    git add integrated_under_empty_top_first.txt
    git commit -m "integrated-under-empty-top: first commit"
    echo "integrated lower commit 2" > integrated_under_empty_top_second.txt
    git add integrated_under_empty_top_second.txt
    git commit -m "integrated-under-empty-top: second commit"
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
  empty-integrated)
    git checkout -b empty-integrated-branch
    echo "empty integrated branch" > empty_integrated_branch.txt
    git add empty_integrated_branch.txt
    git commit -m "empty-integrated: branch commit"
    git checkout master
    git merge --no-ff -m "empty-integrated: merge branch" empty-integrated-branch
    ;;
  local-only-empty)
    git checkout -b local-only-empty-source
    echo "local only empty branch" > local_only_empty_branch.txt
    git add local_only_empty_branch.txt
    git commit -m "local-only-empty: branch commit"
    git checkout master
    git merge --no-ff -m "local-only-empty: merge branch" local-only-empty-source
    git branch -D local-only-empty-source
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
  empty-top-over-integrated)
    "$BUT" apply integrated-branch-under-empty-top
    git checkout integrated-branch-under-empty-top
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
  empty-integrated)
    "$BUT" apply empty-integrated-branch
    git checkout empty-integrated-branch
    ;;
  local-only-empty)
    git branch local-only-empty-branch master^2
    "$BUT" apply local-only-empty-branch
    git checkout local-only-empty-branch
    ;;
esac
popd
