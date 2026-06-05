#!/bin/bash

# Reproduction scaffold for PR #13904 follow-up.
#
# Sets up a managed workspace with a single applied branch (`feature`),
# then deletes `refs/heads/feature` outside of GitButler. The workspace
# commit still has the feature tip as a merge parent so the graph keeps
# emitting the stack — but with both:
#
#   * `Stack.id === null`           (find_matching_stack_id has no metadata
#     ref to match — virtual_branches.toml reconciliation can't recover an
#     id because the ref it indexed by is gone.)
#   * `Segment.refName === null`    (the tip ref no longer points at a
#     reachable commit, see crates/but-graph/src/segment.rs:304-306.)
#
# We've confirmed this against the running but-server: `head_info` returns
# both fields as null on this scenario.

echo "GIT CONFIG $GIT_CONFIG_GLOBAL"
echo "DATA DIR $E2E_TEST_APP_DATA_DIR"
echo "BUT $BUT"

mkdir remote-project
pushd remote-project
  git init -b master --object-format=sha1
  echo "base" >> a_file
  git add a_file
  git commit -am "Base commit on master"

  git checkout -b feature
  echo "feature content" >> b_file
  git add b_file
  git commit -am "feature: add b_file"
  echo "more feature content" >> b_file
  git commit -am "feature: extend b_file"

  git checkout master
popd

git clone remote-project local-clone
pushd local-clone
  git checkout master
  target_branch="$(git rev-parse --symbolic-full-name @{u})"
  target_branch="${target_branch#refs/remotes/}"
  "$BUT" setup
  "$BUT" config target "$target_branch"

  "$BUT" apply feature

  # Yank the local ref out from under the workspace.
  git update-ref -d refs/heads/feature
popd
