#!/usr/bin/env bash

### Description
# A commit with an executable, a normal file, a symlink and an untracked fifo.
# The worktree changes the executable to non-executable,
set -eu -o pipefail

git init
echo content > soon-executable
echo exe > soon-not-executable
chmod +x soon-not-executable 2>/dev/null || true
if ln -s nonexisting-target soon-file-not-link 2>/dev/null; then
  :
else
  printf '%s' nonexisting-target >soon-file-not-link
fi

git add .
git update-index --chmod=+x soon-not-executable
link_oid=$(printf '%s' nonexisting-target | git hash-object -w --stdin)
printf "120000 %s 0\tsoon-file-not-link\n" "$link_oid" | git update-index --index-info
git commit -m "init"
chmod +x soon-executable 2>/dev/null || true
chmod -x soon-not-executable 2>/dev/null || true
rm soon-file-not-link && echo "ordinary content" >soon-file-not-link
