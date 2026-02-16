#!/usr/bin/env bash
set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

init-repo-with-files-and-remote

# Scenario 1: A commit that can be cleanly cherry-picked onto both foo & bar stacks
git branch existing-branch

git checkout -b foo
  echo "foo line 1" > foo.txt
  commit "foo: add line 1"
  echo "foo line 2" >> foo.txt
  commit "foo: add line 2"

git checkout -b bar main
  echo "bar line 1" >> bar.txt
  commit "bar: add line 1"
  echo "bar line 2" >> bar.txt
  commit "bar: add line 2"

# Create an unapplied branch with a commit that modifies shared.txt (no conflicts)
git checkout -b clean-commit-branch main
  echo "clean change" >> shared.txt
  git add . && git commit -m "Add clean change to shared.txt"
  git tag clean-commit

  # Store this as a GitButler reference for easy access
  git update-ref refs/gitbutler/clean-commit clean-commit

git checkout foo
create_workspace_commit_once foo bar
