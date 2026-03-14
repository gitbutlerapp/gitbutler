#!/usr/bin/env bash
set -eu -o pipefail
git init --initial-branch=main repo
(cd repo
  git config user.name "Author"
  git config user.email "author@example.com"
  echo a > file
  git add . && git commit -m "init"
)

# Setup:
# * 6a0c4bd (HEAD -> gitbutler/workspace) GitButler Workspace Commit
# * 95d4a63 foobar
# | * 1e2a3a8 (right) right
# |/
# | * f3d2634 (left) left
# |/
# * 7950f06 (origin/main, origin/HEAD, main) init
# Where "left" and "right" contain changes which conflict with each other
git clone repo conficted_entries_get_written_when_leaving_edit_mode
(cd conficted_entries_get_written_when_leaving_edit_mode
  git config user.name "Author"
  git config user.email "author@example.com"
  git checkout -b left
  echo left > conflict
  git add . && git commit -m "left"
  git checkout main
  git checkout -b right
  echo right > conflict
  git add . && git commit -m "right"
  git checkout main
  git checkout -b branchy
  echo b > file
  git add file && git commit -m "foobar"
  git checkout -b gitbutler/workspace
  git commit --allow-empty -m "GitButler Workspace Commit"
)

git clone repo enter_edit_mode_with_conflicted_commit
(cd enter_edit_mode_with_conflicted_commit
  git config user.name "Author"
  git config user.email "author@example.com"

  git checkout -b branchy
  echo b > file
  git add file && git commit -m "foobar"
  git checkout -b gitbutler/workspace
  git commit --allow-empty -m "GitButler Workspace Commit"

  base_blob=$(printf "base\n" | git hash-object -wt blob --stdin)
  ours_blob=$(printf "left\n" | git hash-object -wt blob --stdin)
  theirs_blob=$(printf "right\n" | git hash-object -wt blob --stdin)
  conflict_files_blob=$(git hash-object -wt blob --stdin <<EOF
ancestorEntries = [ "conflict" ]
ourEntries = [ "conflict" ]
theirEntries = [ "conflict" ]
EOF
  )

  git update-index --add --cacheinfo 100644 "$ours_blob" ".auto-resolution/conflict"
  git update-index --add --cacheinfo 100644 "$base_blob" ".conflict-base-0/conflict"
  git update-index --add --cacheinfo 100644 "$conflict_files_blob" ".conflict-files"
  git update-index --add --cacheinfo 100644 "$ours_blob" ".conflict-side-0/conflict"
  git update-index --add --cacheinfo 100644 "$theirs_blob" ".conflict-side-1/conflict"

  conflict_tree=$(git write-tree)

  conflict_commit=$(git hash-object -wt commit --stdin <<EOF
tree $conflict_tree
parent $(git rev-parse HEAD~)
author Author <author@example.com> 1730625617 +0100
committer Author <author@example.com> 1730625617 +0100
gitbutler-headers-version 2
change-id 00000000-0000-0000-0000-000000000001
gitbutler-conflicted 1

Changes to make millions

EOF
  )

  new_head=$(git commit-tree HEAD^{tree} -p $conflict_commit -m "Gitbutler Workspace Commit")

  git reset --hard $new_head

  git tag conflicted-target $conflict_commit
)
