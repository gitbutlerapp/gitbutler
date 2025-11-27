#!/bin/bash

set -eu -o pipefail

git init four-commits
(cd four-commits
  echo "base" >base && git add . && git commit -m "base"
  echo "a" >a && git add . && git commit -m "a"
  echo "b" >b && git add . && git commit -m "b"
  echo "c" >c && git add . && git commit -m "c"
)

git init many-references
(cd many-references
  echo "base" >base && git add . && git commit -m "base"
  echo "a" >a && git add . && git commit -m "a"
  git branch X
  git branch Y
  git branch Z
  echo "b" >b && git add . && git commit -m "b"
  echo "c" >c && git add . && git commit -m "c"
)

git init three-branches-merged
(cd three-branches-merged
  seq 50 60 >file && git add . && git commit -m "base" && git tag base
  git branch B
  git branch C

  git checkout -b A
  { seq 10; seq 50 60; } >file && git add . && git commit -m "A: 10 lines on top"

  git checkout B
  { seq 50 60; seq 61 70; } >file && git add . && git commit -m "B: 10 lines at the bottom"
  { seq 50 60; seq 61 80; } >file && git add . && git commit -m "B: another 10 lines at the bottom"

  git checkout C
  seq 10 >new-file && git add . && git commit -m "C: new file with 10 lines"
  seq 20 >new-file && git add . && git commit -m "C: add 10 lines to new file"
  seq 30 >new-file && git add . && git commit -m "C: add another 10 lines to new file"

  git checkout main
  git merge A B C
)

git init merge-in-the-middle
(cd merge-in-the-middle
  seq 50 60 >file && git add . && git commit -m "base" && git tag base
  git branch B

  git checkout -b A
  { seq 10; seq 50 60; } >file && git add . && git commit -m "A: 10 lines on top"
  git branch with-inner-merge

  git checkout B
  seq 10 >new-file && git add . && git commit -m "C: new file with 10 lines"

  git checkout with-inner-merge && git merge --no-ff B
  echo seq 10 >'added-after-with-inner-merge' && git add . && git commit -m "on top of inner merge"
)

git init cherry-pick-scenario
(cd cherry-pick-scenario
  echo "a" > base-f && git add . && git commit -m "base" && git branch base

  git checkout -b single-clean-parent
  echo "clean" > clean-f && git add . && git commit -m "single-clean-parent"

  git checkout -b single-clean-commit
  echo "clean-commit" > clean-commit-f && git add . && git commit -m "single-clean-commit"

  git checkout -b single-target
  git reset --hard base
  echo "target" > target-f && git add . && git commit -m "single-target"

  git checkout -b single-conflicting-commit
  git reset --hard single-clean-parent
  echo "conflict" > target-f && git add . && git commit -m "single-conflicting-commit"

  git checkout -b second-target
  git reset --hard base
  echo "target 2" > target-2-f && git add . && git commit -m "second-target"

  git checkout -b second-conflicting-target
  git reset --hard base
  echo "target 2" > target-f && git add . && git commit -m "second-conflicting-target"

  git checkout -b second-clean-parent
  git reset --hard base
  echo "clean 2" > clean-2-f && git add . && git commit -m "second-clean-parent"

  git checkout -b merge-clean-commit
  git merge single-clean-parent
  echo "clean-commit" > clean-commit-f && git add . && git commit -m "merge-clean-commit" --amend

  git checkout -b merge-conflicting-commit
  rm clean-commit-f && echo "conflict" > target-f && git add . && git commit -m "merge-conflicting-commit" --amend

  git checkout -b second-conflicting-parent
  git reset --hard base
  echo "conflict" > clean-f && git add . && git commit -m "second-conflicting-parent"

  git checkout -b merge-clean-commit-conflicting-parents
  set +e
  git merge single-clean-parent --no-edit
  set -e
  echo "resolved" > clean-f
  git add .
  git commit -m "foo"
  echo "clean-commit" > clean-commit-f && git add . && git commit -m "merge-clean-commit-conflicting-parents" --amend

  git checkout -b base-conflicting
  git reset --hard base
  echo "conflict" > target-f && git add . && git commit -m "base-conflicting" --amend 
)