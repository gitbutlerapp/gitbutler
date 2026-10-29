#!/usr/bin/env bash

source "${BASH_SOURCE[0]%/*}/shared.sh"

function rewrite_ref_with_change_id() {
  local ref_name="${1:?ref name}"
  local old_commit
  local new_commit
  old_commit=$(git rev-parse "$ref_name")
  new_commit=$(
    git cat-file commit "$old_commit" \
      | awk '
          !inserted && $0 == "" {
            print "gitbutler-headers-version 2"
            print "change-id 1"
            inserted = 1
          }
          { print }
        ' \
      | git hash-object -t commit -w --stdin
  )
  git update-ref "$ref_name" "$new_commit"
}

git-init-frozen
commit-file M
setup_target_to_match_main

git checkout -b A
commit-file only-on-remote
rewrite_ref_with_change_id refs/heads/A
git update-ref refs/remotes/origin/A refs/heads/A
git reset --hard HEAD~1

commit-file only-on-local
rewrite_ref_with_change_id refs/heads/A
create_workspace_commit_once main A
