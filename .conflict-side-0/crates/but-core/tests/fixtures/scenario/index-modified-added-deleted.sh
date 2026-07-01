#!/usr/bin/env bash

### Description
# A commit with an executable, a normal file, and an untracked fifo,
# which subsequently get modified and deleted in the index, with the symlink being added to it.
set -eu -o pipefail

git init
echo content > modified-content
echo change-exe-bit > modified-exe
echo "soon deleted in index but left on disk" > deleted-exe && chmod +x deleted-exe
mkfifo fifo-should-be-ignored

git add . && git commit -m "init"

echo index-content >modified-content && \
  git add modified-content && \
  echo content >modified-content

ln -s only-in-index link && \
  git add link && \
  rm link

git rm --cached deleted-exe

chmod +x modified-exe && \
  git add modified-exe && \
  chmod -x modified-exe

