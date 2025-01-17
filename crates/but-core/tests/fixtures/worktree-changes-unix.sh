#!/bin/bash

set -eu -o pipefail

git init untracked-fifo
(cd untracked-fifo
  git commit -m "empty" --allow-empty
  mkfifo named-pipe
)

git init add-executable-bit-in-worktree
(cd add-executable-bit-in-worktree
  touch exe && git add exe
  git commit -m "init"
  chmod +x exe
)

git init remove-executable-bit-in-worktree
(cd remove-executable-bit-in-worktree
  touch exe && chmod +x exe && git add exe
  git commit -m "init"
  chmod -x exe
)

git init add-executable-bit-in-index
(cd add-executable-bit-in-index
  touch exe && git add exe
  git commit -m "init"
  chmod +x exe && git add exe
)

git init remove-executable-bit-in-index
(cd remove-executable-bit-in-index
  touch exe && chmod +x exe && git add exe
  git commit -m "init"
  chmod -x exe && git add exe
)

git init symlink-to-file-in-worktree
(cd symlink-to-file-in-worktree
  touch target && ln -s target symlink-soon-file
  git add . && git commit -m "init"
  rm symlink-soon-file && echo content >symlink-soon-file
)

cp -Rv symlink-to-file-in-worktree symlink-to-file-in-index
(cd symlink-to-file-in-index
  git add .
)

git init file-to-symlink-in-worktree
(cd file-to-symlink-in-worktree
  echo content >file-soon-symlink
  git add . && git commit -m "init"
  rm file-soon-symlink && ln -s does-not-exist file-soon-symlink
)

cp -Rv file-to-symlink-in-worktree file-to-symlink-in-index
(cd file-to-symlink-in-index
  git add .
)
