#!/usr/bin/env bash

### Description
# A commit with an executable, a normal file, a symlink and an untracked fifo.
# The worktree changes the executable to non-executable,
set -eu -o pipefail

git init
echo content > soon-executable
echo exe > soon-not-executable && chmod +x soon-not-executable
ln -s nonexisting-target soon-file-not-link
mkfifo fifo-should-be-ignored

git add . && git commit -m "init"
chmod +x soon-executable
chmod -x soon-not-executable
rm soon-file-not-link && echo "ordinary content" >soon-file-not-link

