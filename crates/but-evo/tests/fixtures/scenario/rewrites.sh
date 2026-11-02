#!/usr/bin/env bash

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

### General Description

# `prec` changes one line of `file`, and every other tag is a commit that could replace it.
git init

seq 1 20 >file
git add file
git commit -m base
git tag base

function change-line-ten() {
  sed 's/^10$/ten/' file >file.tmp
  mv file.tmp file
  git add file
}

change-line-ten
git commit -m change
git tag prec

git tag with-header "$(with-header custom-header value)"
git tag with-change-id "$(with-header change-id zzzzzzzz)"
git tag with-legacy-change-id "$(with-header gitbutler-change-id zzzzzzzz)"
git tag with-headers-version "$(with-header gitbutler-headers-version 2)"

git commit --amend -m reworded
git tag reworded

git reset --hard base
sed 's/^10$/TEN/' file >file.tmp
mv file.tmp file
git add file
git commit -m change
git tag amended

git reset --hard base
change-line-ten
GIT_AUTHOR_NAME=someone-else git commit -m change
git tag other-author

git reset --hard base
change-line-ten
GIT_AUTHOR_DATE="2001-02-03 04:05:06 +0000" git commit -m change
git tag other-date

git reset --hard base
touch unrelated
git add unrelated
git commit -m "unrelated parent"
change-line-ten
git commit -m change
git tag rebased-over-unrelated-file

git reset --hard base
{ echo new-top; cat file; } >file.tmp
mv file.tmp file
git add file
git commit -m "shift all lines"
change-line-ten
git commit -m change
git tag rebased-with-shifted-lines

git reset --hard base
sed 's/^9$/nine/' file >file.tmp
mv file.tmp file
git add file
git commit -m "change the line of context"
change-line-ten
git commit -m change
git tag rebased-over-context-change

git reset --hard base
git mv file renamed
sed 's/^10$/ten/' renamed >renamed.tmp
mv renamed.tmp renamed
git add renamed
git commit -m change
git tag renames-the-file

git reset --hard base
change-line-ten
git update-index --chmod=+x file
git commit -m change
git tag also-executable

git reset --hard base
change-line-ten
echo more >another-file
git add another-file
git commit -m change
git tag also-adds-a-file

git reset --hard base
git commit --allow-empty -m empty
git tag empty
git reset --hard prec
