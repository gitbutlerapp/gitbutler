#!/usr/bin/env bash
set -eu -o pipefail

# Initial state for verifying that dumping a linked worktree follows the
# `.git` indirection and writes a real `.git/` directory in the archive. The
# main worktree and linked worktree both have untracked, modified, and staged
# files so the test can verify the dump contains only the linked worktree's
# `HEAD`, worktree, and per-worktree index state.
git init main
(
  cd main
  printf "*.ignored\nignored-dir/\n" >.gitignore
  printf "tracked ignored" >tracked.ignored
  git add .gitignore
  git add -f tracked.ignored
  git commit -m "initial"
  git worktree add -b linked ../linked

  printf "main modified" >tracked.ignored
  printf "main added to index" >main-worktree-added-to-index.txt
  git add main-worktree-added-to-index.txt
  printf "main untracked" >main-worktree-untracked.txt
)
(
  cd linked
  printf "linked modified" >tracked.ignored
  printf "added to index" >linked-worktree-added-to-index.txt
  git add linked-worktree-added-to-index.txt
  printf "untracked" >linked-worktree-untracked.txt
)
