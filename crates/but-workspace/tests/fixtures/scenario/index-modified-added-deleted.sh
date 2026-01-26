#!/usr/bin/env bash

### Description
# A commit with an executable, a normal file, and an untracked fifo,
# which subsequently get modified and deleted in the index, with the symlink being added to it.
set -eu -o pipefail

git init
echo content > modified-content
echo change-exe-bit > modified-exe
echo "soon deleted in index but left on disk" > deleted-exe

git add .
git update-index --chmod=+x deleted-exe
git commit -m "init"

echo index-content >modified-content && \
  git add modified-content && \
  echo content >modified-content

link_oid=$(printf '%s' only-in-index | git hash-object -w --stdin)
printf "120000 %s 0\tlink\n" "$link_oid" | git update-index --index-info

git rm --cached deleted-exe

git update-index --chmod=+x modified-exe
