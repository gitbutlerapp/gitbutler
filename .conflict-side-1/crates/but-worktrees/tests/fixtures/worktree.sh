#!/usr/bin/env bash
set -eu -o pipefail
CLI=${1:?The first argument is the but CLI}

git init remote
(cd remote
  git config user.name "Author"
  git config user.email "author@example.com"

  echo "initial content" > file.txt
  git add . && git commit -m "init"
)

export GITBUTLER_CLI_DATA_DIR=../user/gitbutler/app-data
export E2E_TEST_APP_DATA_DIR=../user/gitbutler/app-data

function tick () {
  if test -z "${tick+set}"; then
    tick=1675176957
  else
    tick=$(($tick + 60))
  fi
  GIT_COMMITTER_DATE="$tick +0100"
  GIT_AUTHOR_DATE="$tick +0100"
  export GIT_COMMITTER_DATE GIT_AUTHOR_DATE
}

# Scenario: Two stacked branches (feature-a, feature-b) and one parallel branch (feature-c)
git clone remote stacked-and-parallel
(cd stacked-and-parallel
  tick
  export CHANGE_ID=851ad981-c986-4bb4-8be9-deadbeefeba2

  git config user.name "Author"
  git config user.email "author@example.com"

  # Initialize GitButler project
  $CLI init

  # Create feature-a stack (base branch)
  $CLI branch new feature-a
  echo "feature-a line 1" >> file.txt
  $CLI commit -m "feature-a: add line 1" feature-a
  echo "feature-a line 2" >> file.txt
  $CLI commit -m "feature-a: add line 2" feature-a

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

# Scenario: Stacked branches
git clone remote stacked-branches
(cd stacked-branches
  tick
  export CHANGE_ID=851ad981-c986-4bb4-8be9-deadbeefeba2

  git config user.name "Author"
  git config user.email "author@example.com"

  # Initialize GitButler project
  $CLI init

  # Create feature-a stack (base branch)
  $CLI branch new feature-a
  echo "feature-a line 1" > foo.txt
  $CLI commit -m "feature-a: add line 1" feature-a
  echo "feature-a line 2" > bar.txt
  $CLI commit -m "feature-a: add line 2" feature-a

  # Create feature-b stack (stacked on feature-a)
  $CLI branch new feature-b --anchor feature-a
  echo "feature-b line 1" > foo.txt
  $CLI commit -m "feature-b: add line 1" feature-b
  echo "feature-b line 2" > bar.txt
  $CLI commit -m "feature-b: add line 2" feature-b
)