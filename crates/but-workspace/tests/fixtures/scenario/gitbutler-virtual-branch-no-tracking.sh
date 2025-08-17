#!/usr/bin/env bash

### General Description

# Simulates GitButler virtual branch scenario where:
# - Local and remote branches exist and have diverged (requiring force push)
# - BUT no tracking relationship is set up (this is what GitButler does)
# - Integration branch is refs/remotes/origin/main
# This tests the fallback logic for detecting force push requirements

set -eu -o pipefail

function set_author() {
  local author=${1:?Author}

  unset GIT_AUTHOR_NAME
  unset GIT_AUTHOR_EMAIL

  git config user.name $author
  git config user.email $author@example.com
}

# Create remote repository
git init remote
(cd remote
  touch file
  git add . && git commit -m init-integration

  git checkout -b A
  touch file-in-A && git add . && git commit -m "new file in A"
  echo remote-change >file-in-A && git commit -am "remote change in A"

  git checkout main
)

# Clone and create GitButler-like scenario
git clone remote gitbutler-no-tracking
(cd gitbutler-no-tracking
  # Create local branch WITHOUT setting up tracking (key difference from standard Git workflow)
  git checkout -b A
  # Reset to simulate GitButler creating branch from an earlier point
  git reset --hard origin/A~1
  
  # Add local changes that diverge from remote
  set_author local-user
  echo local-change >file-in-A && git commit -am "local change in A"
  
  # Now we have:
  # - Local branch A with local commits
  # - Remote origin/A with different commits  
  # - NO tracking relationship between them (GitButler scenario)
  # - Integration branch is refs/remotes/origin/main
  
  # Verify no tracking is set up
  if git config branch.A.remote 2>/dev/null || git config branch.A.merge 2>/dev/null; then
    echo "Error: Tracking should not be set up for this scenario" >&2
    exit 1
  fi
)