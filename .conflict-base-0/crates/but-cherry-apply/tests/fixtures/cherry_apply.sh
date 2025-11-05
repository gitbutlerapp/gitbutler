#!/usr/bin/env bash
set -eu -o pipefail
CLI=${1:?The first argument is the GitButler CLI}

git init remote
(cd remote
  git config user.name "Author"
  git config user.email "author@example.com"

  echo "initial content" > shared.txt
  echo "foo content" > foo.txt
  echo "bar content" > bar.txt
  git add . && git commit -m "init"
)

GITBUTLER_CHANGE_ID=0
function commit_stack() {
  local stack="${1:?}"
  local message="${2:?}"
  ((GITBUTLER_CHANGE_ID += 1))
  GITBUTLER_CHANGE_ID=$GITBUTLER_CHANGE_ID $CLI branch commit "$stack" -m "$message"
}

export GITBUTLER_CLI_DATA_DIR=../user/gitbutler/app-data

# Scenario 1: A commit that can be cleanly cherry-picked onto both foo & bar stacks
git clone remote clean-to-both
(cd clean-to-both
  git config user.name "Author"
  git config user.email "author@example.com"

  git branch existing-branch
  $CLI project add --switch-to-workspace "$(git rev-parse --symbolic-full-name @{u})"

  # Create foo stack
  $CLI branch create --set-default foo
  echo "foo line 1" > foo.txt
  commit_stack "foo" "foo: add line 1"
  echo "foo line 2" >> foo.txt
  commit_stack "foo" "foo: add line 2"

  # Create bar stack
  $CLI branch create --set-default bar
  echo "bar line 1" >> bar.txt
  commit_stack "bar" "bar: add line 1"
  echo "bar line 2" >> bar.txt
  commit_stack "bar" "bar: add line 2"

  # Create an unapplied branch with a commit that modifies shared.txt (no conflicts)
  git checkout -b clean-commit-branch existing-branch
  echo "clean change" >> shared.txt
  git add . && git commit -m "Add clean change to shared.txt"
  git tag clean-commit

  # Store this as a GitButler reference for easy access
  git update-ref refs/gitbutler/clean-commit clean-commit

  # Switch back to workspace
  git checkout gitbutler/workspace
)

# Scenario 2: A commit that conflicts when cherry-picked onto bar (but not foo)
git clone remote conflicts-with-bar
(cd conflicts-with-bar
  git config user.name "Author"
  git config user.email "author@example.com"

  git branch existing-branch
  $CLI project add --switch-to-workspace "$(git rev-parse --symbolic-full-name @{u})"

  # Create foo stack - modifies foo.txt only
  $CLI branch create --set-default foo
  echo "foo line 1" > foo.txt
  commit_stack "foo" "foo: add line 1"
  echo "foo line 2" >> foo.txt
  commit_stack "foo" "foo: add line 2"

  # Create bar stack - modifies bar.txt in a way that will conflict
  $CLI branch create --set-default bar
  echo "bar line 1" > bar.txt
  commit_stack "bar" "bar: overwrite content"
  echo "bar line 2" >> bar.txt
  commit_stack "bar" "bar: add line 2"

  # Create an unapplied branch with a commit that modifies bar.txt differently
  git checkout -b bar-conflict-branch existing-branch
  echo "conflicting bar change" > bar.txt
  git add . && git commit -m "Conflicting change to bar.txt"
  git tag bar-conflict

  # Store this as a GitButler reference
  git update-ref refs/gitbutler/bar-conflict bar-conflict

  # Switch back to workspace
  git checkout gitbutler/workspace
)

# Scenario 3: A commit that conflicts when cherry-picked onto either foo or bar
git clone remote conflicts-with-both
(cd conflicts-with-both
  git config user.name "Author"
  git config user.email "author@example.com"

  git branch existing-branch
  $CLI project add --switch-to-workspace "$(git rev-parse --symbolic-full-name @{u})"

  $CLI branch create --set-default foo
  echo "foo line 1" > foo.txt
  commit_stack "foo" "foo: add line 1"
  echo "foo line 2" >> foo.txt
  commit_stack "foo" "foo: add line 2"

  $CLI branch create --set-default bar
  echo "bar line 1" > bar.txt
  commit_stack "bar" "bar: overwrite content"
  echo "bar line 2" >> bar.txt
  commit_stack "bar" "bar: add line 2"

  # Create an unapplied branch with a commit that modifies shared.txt in yet another way
  git checkout -b both-conflict-branch existing-branch
  echo "conflicting foo change" > foo.txt
  echo "conflicting bar change" > bar.txt
  git add . && git commit -m "Conflicting changes"
  git tag both-conflict

  # Store this as a GitButler reference
  git update-ref refs/gitbutler/both-conflict both-conflict

  # Switch back to workspace
  git checkout gitbutler/workspace
)

# Scenario 4: A workspace with no applied stacks
git clone remote no-stacks
(cd no-stacks
  git config user.name "Author"
  git config user.email "author@example.com"

  git branch existing-branch
  $CLI project add --switch-to-workspace "$(git rev-parse --symbolic-full-name @{u})"

  # Create an unapplied branch with a commit to test cherry-picking
  git checkout -b no-stacks-branch existing-branch
  echo "some change" > shared.txt
  git add . && git commit -m "Change to shared.txt"
  git tag no-stacks-commit

  # Store this as a GitButler reference
  git update-ref refs/gitbutler/no-stacks-commit no-stacks-commit

  # Switch back to workspace (which has no stacks)
  git checkout gitbutler/workspace
)
