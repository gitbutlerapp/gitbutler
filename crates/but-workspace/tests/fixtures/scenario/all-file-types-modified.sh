#!/usr/bin/env bash

### Description
# A commit with an executable, a normal file, a symlink and an untracked fifo.
# All of them get changed in the worktree.
set -eu -o pipefail

git init
seq 5 8 >file
seq 1 3 >executable
chmod +x executable 2>/dev/null || true
if ln -s nonexisting-target link 2>/dev/null; then
  :
else
  printf '%s' nonexisting-target >link
fi

git add .
git update-index --chmod=+x executable
link_oid=$(printf '%s' nonexisting-target | git hash-object -w --stdin)
printf "120000 %s 0\tlink\n" "$link_oid" | git update-index --index-info
git commit -m "init"

seq 5 10 >file
seq 1 5 >executable

rm link
if ln -s other-nonexisting-target link 2>/dev/null; then
  :
else
  printf '%s' other-nonexisting-target >link
fi
