#!/usr/bin/env bash

### General Description

# Various directories with different scenarios for testing stack information *with* or *without* a workspace commit.
set -eu -o pipefail

function remote_tracking_caught_up() {
  setup_remote_tracking "$1"
}

function setup_remote_tracking() {
  local branch_name="${1:?}"
  local remote_branch_name=${2:-"$branch_name"}
  local mode=${3:-"cp"}
  mkdir -p .git/refs/remotes/origin

  if [[ "$mode" == "cp" ]]; then
    cp ".git/refs/heads/$branch_name" ".git/refs/remotes/origin/$remote_branch_name"
  else
    mv ".git/refs/heads/$branch_name" ".git/refs/remotes/origin/$remote_branch_name"
  fi
}

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

function setup_target_to_match_main() {
  remote_tracking_caught_up main

  cat <<EOF >>.git/config
[remote "origin"]
	url = ./fake/local/path/which-is-fine-as-we-dont-fetch-or-push
	fetch = +refs/heads/*:refs/remotes/origin/*

[branch "main"]
  remote = "origin"
  merge = refs/heads/main
EOF
}


# can only be called once per test setup
function create_workspace_commit_once() {
  local workspace_commit_subject="GitButler Workspace Commit"

  if [ $# == 1 ]; then
    local current_branch=$(git rev-parse --abbrev-ref HEAD)
    if [[ "$current_branch" != "$1" ]]; then
      echo "BUG: Must assure the current branch is the branch passed as argument: $current_branch != $1"
      return 42
    fi
  fi

  git checkout -b gitbutler/workspace
  if [ $# == 1 ] || [ $# == 0 ]; then
    git commit --allow-empty -m "$workspace_commit_subject"
  else
    git merge --no-ff -m "$workspace_commit_subject" "${@}"
  fi
}

# can only be called once per test setup, and definitely doesn't do anything smart like the above version.
# TODO: Both yield different results due to the way the merge is done, so that's maybe something to double-check as well.
function create_workspace_commit_aggressively() {
  local workspace_commit_subject="GitButler Workspace Commit"

  if [ $# == 1 ]; then
    local current_branch=$(git rev-parse --abbrev-ref HEAD)
    if [[ "$current_branch" != "$1" ]]; then
      echo "BUG: Must assure the current branch is the branch passed as argument: $current_branch != $1"
      return 42
    fi
  fi

  git checkout -b gitbutler/workspace main
  if [ $# == 1 ] || [ $# == 0 ]; then
    git commit --allow-empty -m "$workspace_commit_subject"
  else
    git merge --no-ff --strategy octopus -m "$workspace_commit_subject" "${@}"
  fi
}

function commit() {
  local message=${1:?first argument is the commit message}
  git commit -m "$message" --allow-empty
}

git init unborn
git init detached
(cd detached
  commit init && git branch other
  commit first && git tag release/v1 && git tag -am "tag object" annotated
)

# A top-down split that is highly unusual, but good to assure we can handle it.
git init multi-root
(cd multi-root
  commit A
  git checkout --orphan B && commit B
  git checkout --orphan C && commit C
  git checkout --orphan D && commit D

  git checkout main && git merge --allow-unrelated-histories B
  git checkout C && git merge --allow-unrelated-histories D

  git checkout main && git merge --allow-unrelated-histories C
)

# A single root that splits up into 4 branches and merges again
git init four-diamond
(cd four-diamond
  commit base
  git checkout -b A && commit A
  git checkout -b B main && commit B
  git checkout -b C main && commit C
  git checkout -b D main && commit D

  git checkout A && git merge B
  git checkout C && git merge D

  git checkout -B merged A && git merge C
)

# A remote reference is seen while traversing another remote.
git init remote-includes-another-remote
(cd remote-includes-another-remote
  commit init
  git checkout -b A
    git branch soon-remote-A
    commit A
  git checkout -b B
    commit B

  git checkout soon-remote-A
    tick
    commit A
    git checkout -b soon-remote-B
    commit B
  setup_remote_tracking soon-remote-A A "move"
  setup_remote_tracking soon-remote-B B "move"

  git checkout B

cat <<EOF >>.git/config
 [remote "origin"]
 	url = .
 	fetch = +refs/heads/*:refs/remotes/origin/*

 [branch "A"]
   remote = "origin"
   merge = refs/heads/A
 [branch "B"]
   remote = "origin"
   merge = refs/heads/B
EOF

)

git init triple-merge
(cd triple-merge
  for c in $(seq 5); do
    commit "$c"
  done
  git checkout -b A
    git branch B
    git branch C
    for c in $(seq 3); do
      commit "A$c"
    done

  git checkout B
    for c in $(seq 3); do
      commit "B$c"
    done

  git checkout C
    for c in $(seq 3); do
      commit "C$c"
    done
  git merge A B
)

mkdir ws
(cd ws
  git init single-stack-ambiguous
  (cd single-stack-ambiguous
     commit init
       setup_target_to_match_main
       git branch new-A
       git branch new-B
     git checkout -b A
       commit segment-A
       for name in A-empty-01 A-empty-02 A-empty-03; do
         git branch "$name"
       done
     git checkout -b B
       commit segment-B~1 && git branch B-empty && git branch ambiguous-01
       commit segment-B && git tag without-ref
       commit with-ref
     create_workspace_commit_once B
  )

  git init single-stack
  (cd single-stack
     commit init
       setup_target_to_match_main
       git branch new-A
     git checkout -b A
       commit segment-A
     git checkout -b B
       commit segment-B~1
         git branch B-sub
       commit segment-B
     create_workspace_commit_once B
  )

  git init dual-merge
  (cd dual-merge
     commit init
       setup_target_to_match_main
       git branch B
     git checkout -b A
       commit A
     git checkout B
       commit B
     git checkout -b merge
       git merge --no-ff A
       git branch empty-1-on-merge
       git branch empty-2-on-merge
     git checkout -b C
       git branch D
       commit C
     git checkout D
       commit D
     git checkout -b merge-2
       git merge --no-ff C
     create_workspace_commit_once merge-2
  )

  cp -rv dual-merge dual-merge-no-refs
  (cd dual-merge-no-refs
    git branch -d merge-2 C D A B merge empty-2-on-merge empty-1-on-merge main
    rm .git/refs/remotes/origin/main
  )

  git init graph-splitting
  (cd graph-splitting
     commit init
     commit other-1
     git checkout -b entrypoint
       commit A
       commit B
       commit C
     git checkout main
     commit other-2
     create_workspace_commit_once main
  )

  git init just-init-with-branches
  (cd just-init-with-branches
    commit init && setup_target_to_match_main
    for name in A B C D E F gitbutler/workspace; do
      git branch "$name"
    done
  )

  # The remote of 'main' is officially setup.
  git init proper-remote-ahead
  (cd proper-remote-ahead
    commit init && setup_target_to_match_main
    commit shared
    git checkout -b soon-remote;
      commit only-remote-01;
      commit only-remote-02;
    git checkout main && create_workspace_commit_once main
    setup_remote_tracking soon-remote main "move"
  )

  # The remote of 'main' is just deduced by name.
  git init deduced-remote-ahead
  (cd deduced-remote-ahead
    commit init
    git checkout -b A
    commit shared
    git checkout -b soon-remote;
      git checkout -b tmp
        commit feat-on-remote
      git checkout soon-remote
      git merge --no-ff -m "merge" tmp && git branch -d tmp
      commit only-remote-01;
      commit only-remote-02;
    git checkout A
      commit A1
      commit A2
    create_workspace_commit_once A
    setup_remote_tracking soon-remote A "move"

cat <<EOF >>.git/config
[remote "origin"]
  url = ./want-just-a-remote-name
  fetch = +refs/heads/*:refs/remotes/origin/*
EOF

  )

  # A remote reference is seen while traversing another remote.
  git init remote-includes-another-remote
  (cd remote-includes-another-remote
    commit init && setup_target_to_match_main
    git checkout -b A
      git branch soon-remote-A
      commit A
    git checkout -b B
      commit B
    create_workspace_commit_once B

    git checkout soon-remote-A
      tick
      commit A
      git checkout -b soon-remote-B
      commit B
    setup_remote_tracking soon-remote-A A "move"
    setup_remote_tracking soon-remote-B B "move"

    git checkout gitbutler/workspace
  )

  git init disambiguate-by-remote
  (cd disambiguate-by-remote
    commit init && setup_target_to_match_main
    git checkout -b A
      commit A
      git branch soon-remote-on-top-of-A
      git branch ambiguous-A
    git checkout -b B
      commit B
      git branch soon-remote-ahead-of-B
      git branch ambiguous-B
    git checkout -b C
      commit C
      git branch soon-remote-on-top-of-C
      git branch ambiguous-C
      git branch soon-remote-on-top-of-ambiguous-C

    create_workspace_commit_once C
    setup_remote_tracking soon-remote-on-top-of-A A "move"
    setup_remote_tracking soon-remote-on-top-of-C C "move"
    setup_remote_tracking soon-remote-on-top-of-ambiguous-C ambiguous-C "move"

    git checkout soon-remote-ahead-of-B
      commit remote-of-B
      setup_remote_tracking soon-remote-ahead-of-B B "move"

    git checkout gitbutler/workspace
  )

  git init two-segments-one-integrated
  (cd two-segments-one-integrated
    for c in $(seq 3); do
      commit "$c"
    done
    git checkout -b A
      commit 4
      git checkout -b A-feat
        commit "A-feat-1"
        commit "A-feat-2"
      git checkout A
      git merge --no-ff A-feat
      for c in $(seq 5 8); do
        commit "$c"
      done
    git checkout -b B
      commit "B1"
      commit "B2"

    create_workspace_commit_once B

    tick
    git checkout -b soon-origin-main main
      git merge --no-ff A
      for c in $(seq 2); do
        commit "remote-$c"
      done
      setup_remote_tracking soon-origin-main main "move"
    git checkout gitbutler/workspace
  )

  git init on-top-of-target-with-history
  (cd on-top-of-target-with-history
    commit outdated-main
    git checkout -b soon-origin-main
    for c in $(seq 5); do
      commit "$c"
    done
    for name in A B C D E F gitbutler/workspace; do
      git branch "$name"
    done
    setup_remote_tracking soon-origin-main main "move"
    git checkout gitbutler/workspace
  )

  # partition 1: main - start of traversal
  # partition 2: workspace - connected to 1 via short route that isn't including the tip of partition 1
  # partition 3: target - connected to 2 via short route and to 1 via longest rout (2 would find 1 first)
  git init gitlab-case
  (cd gitlab-case
    # there is along tail of history under main which we should be able to traverse as well the entrypoint permits.
    commit M1
    commit M2
    commit M3
    commit M4
    commit M5
    commit M6
    commit M7
    commit M8
    commit M9
    commit M10
    # short link to the workspace, connects to 'main'
    git checkout -b main-to-workspace
      commit Ws1

    git checkout main
    commit M2

    # the long link to the workspace, through 'main'
    git checkout -b long-main-to-workspace main
      commit Wl1
      commit Wl2
      commit Wl3
      commit Wl4

    # workspace finds 'main' through short leg.
    git checkout -b workspace main-to-workspace
    git merge -m "W1-merge" --no-ff long-main-to-workspace
    # NOTE: could have multiple lanes, to be done later for realism.
    git checkout -b workspace-to-target
      commit Ts1
      commit Ts2
      commit Ts3
    git checkout -b long-workspace-to-target workspace
      commit Tl1
      commit Tl2
      commit Tl3
      commit Tl4
      commit Tl5
      commit Tl6
      commit Tl7
    git checkout -b soon-remote-main workspace-to-target
      git merge -m "target" --no-ff long-workspace-to-target
    git checkout workspace
    # This creates a workspace commit outside of the workspace, it can't be reached by the target.
    create_workspace_commit_once workspace

    setup_remote_tracking soon-remote-main main "move"
  )

  # like above, but triggers a different case where 'main' can't be reached easily.
  git init gitlab-case2
  (cd gitlab-case2
    commit M1
    # short link to the workspace, connects to 'main'
    git checkout -b main-to-workspace
      commit Ws1
    git checkout -b longer-workspace-to-target
      commit Tll1
      commit Tll2
      commit Tll3
      commit Tll4
      commit Tll5
      commit Tll6

    git checkout main
    commit M2

    # the long link to the workspace, through 'main'
    git checkout -b long-main-to-workspace main
      commit Wl1
      commit Wl2
      commit Wl3
      commit Wl4

    # workspace finds 'main' through short leg.
    git checkout -b workspace main-to-workspace
    git merge -m "W1-merge" --no-ff long-main-to-workspace
    # NOTE: could have multiple lanes, to be done later for realism.
    git checkout -b long-workspace-to-target workspace
      commit Tl1
      git merge -m "Tl-merge" --no-ff longer-workspace-to-target
      commit Tl2
      commit Tl3
      commit Tl4
      commit Tl5
      commit Tl6
      commit Tl7
      commit Tl8
      commit Tl9
      commit Tl10
    # target is connected through a long leg that takes longer than everything else
    git checkout -b soon-remote-main long-workspace-to-target
    git checkout workspace
    # This creates a workspace commit outside of the workspace, it can't be reached by the target.
    create_workspace_commit_once workspace

    setup_remote_tracking soon-remote-main main "move"
  )
)

