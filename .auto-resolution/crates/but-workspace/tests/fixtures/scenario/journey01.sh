#!/usr/bin/env bash

### General Description

# Various stages through a typical user journey. Ideally complete enough to test everything that matters to us.
set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

git init 01-unborn
(cd 01-unborn
  echo "a newly initialized repository" >.git/description
)

git init 02-first-commit
(cd 02-first-commit
  echo "the root commit is now present locally" >.git/description
  commit init
)

cp -R 02-first-commit 03-main-pushed
(cd 03-main-pushed
  echo "main was pushed so it can now serve as target.

However, without an official workspace it still won't be acting as a target." >.git/description
  setup_target_to_match_main
)

cp -R 03-main-pushed 04-create-workspace
(cd 04-create-workspace
  echo "An official workspace was created, with nothing in it" >.git/description
  create_workspace_commit_once main
)

cp -R 04-create-workspace 05-empty-stack
(cd 05-empty-stack
  echo "an empty stack with nothing in it" >.git/description

  git branch S1 gitbutler/workspace~1
)

git init 06-create-commit-in-stack
(cd 06-create-commit-in-stack
  echo "Create a new commit in the newly added stack S1" >.git/description
  commit init && setup_target_to_match_main
  git checkout -b S1
    commit one
  create_workspace_commit_once S1
)

cp -R 06-create-commit-in-stack 07-push-commit
(cd 07-push-commit
  echo "push S1 to the remote which is then up-to-date" >.git/description
  git rev-parse S1 > .git/refs/remotes/origin/S1
)

git init 08-new-local-commit
(cd 08-new-local-commit
  echo "Create a new local commit right after the previous pushed one

  This leaves the stack in a state where it can be pushed.
  " >.git/description
  commit init && setup_target_to_match_main
  git checkout -b S1
    commit one
    git rev-parse @ > .git/refs/remotes/origin/S1
    commit two
  create_workspace_commit_once S1
)

git init 09-rewritten-local-commit
(cd 09-rewritten-local-commit
  echo "The new local commit was rewritten after pushing it to the remote" >.git/description
  commit init && setup_target_to_match_main
  git checkout -b S1
    echo hi >other && git add other && git commit -m one
    git branch soon-S1-remote
    echo hi >file && git add file && git commit -m two
  git checkout soon-S1-remote
    tick
    git cherry-pick S1
    setup_remote_tracking soon-S1-remote S1 "move"
  git checkout S1
  create_workspace_commit_once S1
)

cp -R 09-rewritten-local-commit 10-squash-merge-stack
(cd 10-squash-merge-stack
  echo "The remote squash-merges S1 *and* changes the 'file' so it looks entirely different in another commit.

  The squash merge should still be detected." >.git/description

  git checkout main
  git merge --squash S1 && git commit -m "squash S1"
  echo overwrite >file && git commit -am "file changed completely afterwards"
  git rev-parse @ > .git/refs/remotes/origin/main
  git reset --hard main~2

  git checkout gitbutler/workspace
)

cp -R 10-squash-merge-stack 11-remote-only
(cd 11-remote-only
  echo "The remote was reused and merged once more with more changes.

  After S1 was squash-merged, someone else reused the branch, pushed two commits
  and squash-merged them into target again.

  Here we assure that these integrated remote commits don't mess with our logic." >.git/description

  git checkout --no-track -b soon-S1-remote origin/S1
    echo new >remote-file && git add remote-file && git commit -m "add remote file"
    echo new >remote-other && git add remote-other && git commit -m "add other remote file"
    setup_remote_tracking soon-S1-remote S1 "move"

  git checkout main
  git reset --hard origin/main
  git revert -n @ && git commit -am "avoid merge conflict"
  git merge --squash origin/S1 && git commit -m "squash origin/S1"
  echo overwrite >remote-other && git commit -am "other remote file changed completely afterwards"
  git rev-parse @ > .git/refs/remotes/origin/main
  git reset --hard main~3

  git checkout gitbutler/workspace
)

cp -R 11-remote-only 12-local-only-multi-segment-squash-merge
(cd 12-local-only-multi-segment-squash-merge
  echo "A new multi-segment stack is created without remote and squash merged locally.

  There is no need to add the local branches to the workspace officially, they are still picked up.
  This allows the user to manually manipulate the workspace and it will work just the same." >.git/description

  git checkout -b local-bottom :/init
    echo new >local-bottom-file && git add local-bottom-file && git commit -m "new local-bottom file"
  git checkout -b local
    echo new >local-file && git add local-file && git commit -m "new local file"
  git branch -D gitbutler/workspace

  git checkout main && git reset --hard origin/main
    git merge --squash local && git commit -m "squash local"
    echo overwrite >local-file && git commit -am "local file rewritten completely"
    git rev-parse @ > .git/refs/remotes/origin/main
    git reset --hard @~2

  git checkout S1
  create_workspace_commit_once S1 local
)
