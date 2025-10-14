#!/usr/bin/env bash
set -eu -o pipefail
CLI=${1:?The first argument is the GitButler CLI}

git init remote
(cd remote
  git config user.name "Author"
  git config user.email "author@example.com"

  echo "initial content" > file.txt
  git add . && git commit -m "init"
)

export GITBUTLER_CLI_DATA_DIR=../user/gitbutler/app-data
export E2E_TEST_APP_DATA_DIR=../user/gitbutler/app-data

# Scenario: Two stacked branches (feature-a, feature-b) and one parallel branch (feature-c)
git clone remote stacked-and-parallel
(cd stacked-and-parallel
  git config user.name "Author"
  git config user.email "author@example.com"

  echo A
  # Initialize GitButler project
  $CLI init
  echo B

  # Create feature-a stack (base branch)
  $CLI branch new feature-a
  echo "feature-a line 1" >> file.txt
  $CLI commit -m "feature-a: add line 1" feature-a
  echo "feature-a line 2" >> file.txt
  $CLI commit -m "feature-a: add line 2" feature-a
  echo C

  # Create feature-b stack (stacked on feature-a)
  $CLI branch new feature-b --anchor feature-a
  echo "feature-b line 1" >> file.txt
  $CLI commit -m "feature-b: add line 1" feature-b
  echo "feature-b line 2" >> file.txt
  $CLI commit -m "feature-b: add line 2" feature-b

  # Create feature-c stack (parallel to feature-a)
  $CLI branch new feature-c
  echo "feature-c content" > feature-c.txt
  $CLI commit -m "feature-c: add new file" feature-c
  echo "feature-c line 2" >> feature-c.txt
  $CLI commit -m "feature-c: add line 2" feature-c
)
