#!/usr/bin/env bash

### Description
# A newly initialized git repository with an executable, a normal file, a symlink and a fifo
set -eu -o pipefail

git init
echo content > untracked
echo exe > untracked-exe && chmod +x untracked-exe
ln -s untracked link
mkdir dir
mkfifo dir/fifo-should-be-ignored

