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
