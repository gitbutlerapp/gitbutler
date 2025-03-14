#!/usr/bin/env bash

### Description
# A commit with an executable, a normal file, a symlink.
# Then each item gets modified and renamed in the worktree, with one overwriting a tracked file and directory.
set -eu -o pipefail

git init
seq 5 8 >file
seq 9 13 >other-file
seq 1 3 >executable && chmod +x executable
ln -s nonexisting-target link

touch file-to-be-dir
mkdir dir-to-be-file && touch dir-to-be-file/content
touch to-be-overwritten

git add . && git commit -m "init"

seq 5 9 >file
seq 9 14 >other-file
seq 1 4 >executable
rm file-to-be-dir && mkdir file-to-be-dir && mv file file-to-be-dir/file
rm -Rf dir-to-be-file && mv executable dir-to-be-file
mv other-file to-be-overwritten

rm link
ln -s other-nonexisting-target link-renamed

