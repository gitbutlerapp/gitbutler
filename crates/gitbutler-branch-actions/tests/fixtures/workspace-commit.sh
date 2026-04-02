#!/usr/bin/env bash
set -eu -o pipefail

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

function commit_exact () {
  local message=${1:?}
  git add -A
  local tree
  tree=$(git write-tree)
  local parent_args=()
  if git rev-parse --verify HEAD >/dev/null 2>&1; then
    parent_args=(-p HEAD)
  fi
  local commit
  commit=$(printf "%s" "$message" | git commit-tree "$tree" "${parent_args[@]}")
  local current_branch
  current_branch=$(git symbolic-ref -q HEAD || true)
  if [[ -n "$current_branch" ]]; then
    git update-ref "$current_branch" "$commit"
  fi
  git reset --hard "$commit" >/dev/null
}

function commit_with_tick () {
  local message=${1:?}
  tick
  commit_exact "$message"
}

git init --initial-branch=main remote
(cd remote
  git config user.name "Author"
  git config user.email "author@example.com"
  echo "base content" > shared.txt
  seq 15 > file
  git add . && git commit -m "init"
)

# Two stacks that both modify shared.txt with conflicting content.
# This triggers a merge conflict in remerged_workspace_tree_v2 (gix),
# which sets the later stack's in_workspace to false.
git clone remote conflicting-stacks
(cd conflicting-stacks
  git config user.name "Author"
  git config user.email "author@example.com"

  git checkout -b stack_a main
  echo "content from stack a" > shared.txt
  commit_with_tick "stack_a commit"

  git checkout -b stack_b main
  echo "content from stack b" > shared.txt
  commit_with_tick "stack_b commit"

  # The workspace commit merges both stacks.
  # remerged_workspace_tree_v2 will detect that they conflict.
  git checkout -b gitbutler/workspace main
  # We can't actually merge conflicting branches, so just point workspace
  # at main. The seed + update_workspace_commit call in the test will
  # rebuild it properly.
)

# Two stacks each modifying adjacent (non-overlapping) sections of the same file,
# with zero lines of buffer between the changed regions.
# Stack A owns lines 1-5 and lines 11-15; Stack B owns lines 6-10.
# A's top hunk immediately precedes B's hunk (adjacency from above), and
# B's hunk immediately precedes A's bottom hunk (adjacency from below).
# This exercises the gix fix for the git2 bug where adjacent hunks in an
# octopus workspace merge were incorrectly flagged as conflicting.
git clone remote adjacent-stacks
(cd adjacent-stacks
  git config user.name "Author"
  git config user.email "author@example.com"

  git checkout -b stack_a main

  # Change lines 1-5 (top) and lines 11-15 (bottom); lines 6-10 untouched.
  printf 'a1\na2\na3\na4\na5\n6\n7\n8\n9\n10\na11\na12\na13\na14\na15\n' > file
  commit_with_tick "stack_a: change top and bottom sections"

  git checkout -b stack_b main
  # Change only lines 6-10 (middle); lines 1-5 and 11-15 untouched from base.
  printf '1\n2\n3\n4\n5\nb6\nb7\nb8\nb9\nb10\n11\n12\n13\n14\n15\n' > file
  commit_with_tick "stack_b: change middle section"

  # Point workspace at main; update_workspace_commit in the test rebuilds it properly.
  git checkout -b gitbutler/workspace main
  git commit --allow-empty -m "GitButler Workspace Commit"
)
