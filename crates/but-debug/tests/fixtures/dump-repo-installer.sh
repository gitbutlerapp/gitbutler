#!/usr/bin/env bash
set -eu -o pipefail

git init --bare -b main remote.git
git init --bare -b main other.git
git init -b main gitbutler
(
  cd gitbutler
  git config user.name gitbutler-test
  git config user.email gitbutler-test@example.com

  printf "base\n" >file.txt
  git add file.txt
  git commit -m "base"
  git remote add origin ../remote.git
  git push -u origin main
  git symbolic-ref refs/remotes/origin/HEAD refs/remotes/origin/main

  git checkout -b other-main
  printf "other remote\n" >other.txt
  git add other.txt
  git commit -m "other remote"
  git remote add other ../other.git
  git push other HEAD:main
  git fetch other

  git checkout -b gitbutler/workspace
  git branch -D other-main
  printf "workspace\n" >file.txt
  git commit -am "workspace"
  git update-ref refs/gitbutler/test-ref HEAD
  git update-ref refs/namespaces/gitbutler-stashes/refs/heads/gitbutler/workspace HEAD^
  git symbolic-ref refs/gitbutler/symbolic refs/gitbutler/test-ref
  git symbolic-ref refs/gitbutler/dangling refs/heads/missing

  reflog_only=$(printf "reflog only\n" | git commit-tree HEAD^{tree})
  printf "%s\n" "$reflog_only" >../reflog-only-oid
  target=$(git rev-parse main)
  git update-ref refs/heads/gitbutler/target "$reflog_only"
  git update-ref refs/heads/gitbutler/target "$target"

  mkdir -p .git/gitbutler/nested
  printf "state marker\n" >.git/gitbutler/nested/state.txt
  printf 'head_sha = "%s"\n' "$target" >.git/gitbutler/operations-log.toml

  git config --local gitbutler.project.targetRef refs/remotes/origin/main
  git config --local gitbutler.project.targetCommitId "$(git rev-parse refs/remotes/origin/main)"
  git config --local gitbutler.project.pushRemote origin
)
