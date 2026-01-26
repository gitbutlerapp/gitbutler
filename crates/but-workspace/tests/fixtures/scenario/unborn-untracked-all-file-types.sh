#!/usr/bin/env bash

### Description
# A newly initialized git repository with an executable, a normal file, a symlink and a fifo
set -eu -o pipefail

git init
echo content > untracked
echo exe > untracked-exe
chmod +x untracked-exe 2>/dev/null || true
if ln -s untracked link 2>/dev/null; then
  :
else
  printf '%s' untracked >link
fi
