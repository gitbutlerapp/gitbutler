#!/usr/bin/env bash
set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/../../../but/tests/fixtures/scenario/shared.sh"

function commit_with_tick () {
  tick
  git add -A
  commit "${1:?}"
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

  stack_a_oid=$(git rev-parse stack_a)
  stack_b_oid=$(git rev-parse stack_b)
  tree=$(git rev-parse main^{tree})
  ws_commit=$(echo "GitButler Workspace Commit" | git commit-tree "$tree" -p "$stack_a_oid" -p "$stack_b_oid")
  git checkout -b gitbutler/workspace "$ws_commit"
)

# Two stacks each modifying nearby, non-overlapping sections of the same file.
# Stack A owns lines 1-5 and lines 11-15; Stack B owns lines 7-9. Lines 6 and
# 10 provide unchanged merge context between the three hunks.
git clone remote adjacent-stacks
(cd adjacent-stacks
  git config user.name "Author"
  git config user.email "author@example.com"

  git checkout -b stack_a main

  # Change lines 1-5 (top) and lines 11-15 (bottom); lines 6-10 untouched.
  printf 'a1\na2\na3\na4\na5\n6\n7\n8\n9\n10\na11\na12\na13\na14\na15\n' > file
  commit_with_tick "stack_a: change top and bottom sections"

  git checkout -b stack_b main
  # Change only lines 7-9 (middle); lines 6 and 10 remain as merge context.
  printf '1\n2\n3\n4\n5\n6\nb7\nb8\nb9\n10\n11\n12\n13\n14\n15\n' > file
  commit_with_tick "stack_b: change middle section"

  git checkout main
  create_workspace_commit_once main
)

git clone remote diverged-stacks
(cd diverged-stacks
  git config user.name "Author"
  git config user.email "author@example.com"

  # A: {a, b, c}
  git rm -q file shared.txt
  echo "a" > a
  echo "b" > b
  echo "c" > c
  commit_with_tick "A: base set"
  git tag base-a

  # B: {x, b, c}; this is the target.
  rm a
  echo "x" > x
  commit_with_tick "B: target replaces a with x"
  git tag target-b

  # C: {x, y, c}
  rm b
  echo "y" > y
  commit_with_tick "C: stack replaces b with y"
  git branch stack_c

  # D: {a, b, z}
  git checkout -b stack_d base-a
  rm c
  echo "z" > z
  commit_with_tick "D: stack replaces c with z"

  git update-ref refs/remotes/origin/main target-b

  stack_c_oid=$(git rev-parse stack_c)
  stack_d_oid=$(git rev-parse stack_d)
  # Start with stack C's incomplete tree so the test must rebuild the workspace merge.
  tree=$(git rev-parse stack_c^{tree})
  ws_commit=$(echo "GitButler Workspace Commit" | git commit-tree "$tree" -p "$stack_c_oid" -p "$stack_d_oid")
  git checkout -b gitbutler/workspace
  git reset --hard "$ws_commit"
)
