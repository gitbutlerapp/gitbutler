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

  local dst=".git/refs/remotes/origin/$remote_branch_name";
  mkdir -p "${dst%/*}"
  if [[ "$mode" == "cp" ]]; then
    cp ".git/refs/heads/$branch_name" $dst
  else
    mv ".git/refs/heads/$branch_name" $dst
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

  add_main_remote_setup
}

function add_main_remote_setup() {
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
  git checkout -f "$(git rev-parse HEAD)"
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

git init ambiguous-worktrees
(cd ambiguous-worktrees
  commit M
  git worktree add ../wt-outside-ambiguous-worktree
  git worktree add wt-inside-ambiguous-worktree
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

git init special-branches
(cd special-branches
  commit init
    git branch gitbutler/target
  commit middle
    git branch gitbutler/edit
  commit top
)

mkdir ws
(cd ws
  git init duplicate-workspace-connection
  (cd duplicate-workspace-connection
    # this repo is for reproducing a real double-connection, which isn't always happening but causes issues downstream.
    commit init
    git branch A
    git branch B
    setup_target_to_match_main
    create_workspace_commit_once main
    commit_with_duplicate_parents=$(git cat-file -p  @ | sed '/parent/ { p; }' | git hash-object -t commit --stdin -w)
    git update-ref refs/heads/gitbutler/workspace "${commit_with_duplicate_parents}"

    git checkout -b soon-origin-main main
      commit RM
    git checkout gitbutler/workspace
    mv .git/refs/heads/soon-origin-main .git/refs/remotes/origin/main
  )

  git init duplicate-workspace-connection-no-target
  (cd duplicate-workspace-connection-no-target
    # like above, but don't let the target be advanced.
    commit init
    git branch A
    git branch B
    setup_target_to_match_main
    create_workspace_commit_once main
    commit_with_duplicate_parents=$(git cat-file -p  @ | sed '/parent/ { p; }' | git hash-object -t commit --stdin -w)
    git update-ref refs/heads/gitbutler/workspace "${commit_with_duplicate_parents}"
  )

  git init ambiguous-worktrees
  (cd ambiguous-worktrees
    commit M1
    commit M-base

    git branch A
    git worktree add -b A-inside wt-A-inside
    git worktree add -b A-outside ../wt-A-outside

    git checkout -b soon-origin-A main
      commit A-remote
    git checkout main
      commit M-advanced
      setup_target_to_match_main

    git checkout -b B A
      commit B
    git checkout A
    git worktree add wt-B-inside B

    create_workspace_commit_once A B
    setup_remote_tracking soon-origin-A A "move"
    git worktree add wt-origin-A-inside origin/A
  )

  git init remote-and-integrated-tracking-linear
  (cd remote-and-integrated-tracking-linear
     commit M1
     commit M-base
     git branch A
     git checkout -b soon-origin-A main
       commit A-remote
     git checkout main
       commit M-advanced
       setup_target_to_match_main

     git checkout A
     create_workspace_commit_once A
     setup_remote_tracking soon-origin-A A "move"
  )

  git init remote-and-integrated-tracking
  (cd remote-and-integrated-tracking
     commit M1
     commit M2
     git checkout -b tmp1
      commit X
     git checkout main
     commit Y
     git merge --no-ff tmp1 -m "M-base"
     git branch A
     git checkout -b soon-origin-A main
       commit A-remote
     git checkout main
       commit M-advanced
       setup_target_to_match_main

     git checkout A
     create_workspace_commit_once A
     setup_remote_tracking soon-origin-A A "move"
  )

  git init remote-and-integrated-tracking-extra-commit
  (cd remote-and-integrated-tracking-extra-commit
     commit M1
     commit M2
     git checkout -b tmp1
      commit X
     git checkout main
     commit Y
     git merge --no-ff tmp1 -m "M-base"
     git checkout -b A
       commit A-local
     git checkout -b soon-origin-A main
       commit A-remote
     git checkout main
       commit M-advanced
       setup_target_to_match_main

     git checkout A
     create_workspace_commit_once A
     setup_remote_tracking soon-origin-A A "move"
  )

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
       git branch soon-origin-B
       commit segment-B~1 && git branch B-empty && git branch ambiguous-01
       commit segment-B && git tag without-ref
       commit with-ref
       setup_remote_tracking soon-origin-B B "move"
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

  git init single-merge-into-main
  (cd single-merge-into-main
     commit init
       git branch B
     git checkout -b A
       commit A
     git checkout B
       commit B
     git checkout -b merge
       git merge --no-ff A
       cp .git/refs/heads/merge .git/refs/heads/main
       setup_target_to_match_main
     git checkout -b C
       commit C
     create_workspace_commit_once C
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

  git init just-init-with-two-branches
  (cd just-init-with-two-branches
    commit init
    git branch A
    git branch B
    git checkout -b gitbutler/workspace
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
    mkdir .git/refs/remotes/push-remote
    cp .git/refs/remotes/origin/A .git/refs/remotes/push-remote/A

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

  git init two-segments-one-integrated-without-remote
  (cd two-segments-one-integrated-without-remote
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

  cp -R two-segments-one-integrated-without-remote two-segments-one-integrated
  (cd two-segments-one-integrated
    add_main_remote_setup
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
    add_main_remote_setup
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
  git init multi-lane-with-shared-segment
  (cd multi-lane-with-shared-segment
    commit M1
    git checkout -b shared
      commit S1
      commit S2
      commit S3
    git checkout -b A
      git branch B
      git branch C
      commit A1
    git checkout B
      commit B1
      commit B2
    git checkout C
      commit C1
      commit C2
      commit C3
    git checkout -b D
      commit D1
    create_workspace_commit_once A B D
    git checkout -b soon-remote-main main
    commit M2

    git checkout gitbutler/workspace
    setup_remote_tracking soon-remote-main main "move"
  )

  git init multi-lane-with-shared-segment-one-integrated
  (cd multi-lane-with-shared-segment-one-integrated
    commit M1
    git checkout -b shared
      commit S1
      commit S2
      commit S3
    git checkout -b A
      git branch B
      git branch C
      commit A1
    git checkout B
      commit B1
      commit B2
    git checkout C
      commit C1
      commit C2
      commit C3
    git checkout -b D
      commit D1
    create_workspace_commit_once A B D
    git checkout -b soon-remote-main main
    git merge --no-ff A

    git checkout gitbutler/workspace
    setup_remote_tracking soon-remote-main main "move"
    add_main_remote_setup
  )

  git init three-branches-one-advanced-ws-commit-advanced-fully-pushed-empty-dependant
  (cd three-branches-one-advanced-ws-commit-advanced-fully-pushed-empty-dependant
    commit "init"
    setup_target_to_match_main
    git checkout -b lane main

    git checkout -b advanced-lane
    commit "change"
    # This works without an official remote setup as we go by name as fallback.
    remote_tracking_caught_up advanced-lane
    git branch dependant
    git branch on-top-of-dependant

    create_workspace_commit_once advanced-lane
  )

  git init two-branches-one-advanced-two-parent-ws-commit-advanced-fully-pushed-empty-dependant
  (cd two-branches-one-advanced-two-parent-ws-commit-advanced-fully-pushed-empty-dependant
    commit "init"
    setup_target_to_match_main
    git checkout -b lane main

    git checkout -b advanced-lane
    commit "change"

    create_workspace_commit_aggressively lane advanced-lane

    remote_tracking_caught_up advanced-lane
    git branch dependant advanced-lane
  )

  # There are multiple stacked branches that could lead towards a shared stack.
  git init multiple-stacks-with-shared-segment-and-remote
  (cd multiple-stacks-with-shared-segment-and-remote
    commit init && setup_target_to_match_main
    git checkout -b A
     commit A
     git checkout -b soon-origin-A
       commit A-on-remote

    git checkout -b B-on-A A
      commit "B-on-A"

    git checkout -b C-on-A A
      commit "C-on-A"

    setup_remote_tracking soon-origin-A A "move"
    create_workspace_commit_once B-on-A C-on-A
  )

  git init two-branches-one-advanced-two-parent-ws-commit-diverged-ttb
  (cd two-branches-one-advanced-two-parent-ws-commit-diverged-ttb
    commit "init"
    git checkout -b lane main

    git checkout -b advanced-lane
    commit "change"

    create_workspace_commit_aggressively advanced-lane lane
    # swap trees - Git puts 'lane' first for some reason, but we really need the other way to reproduce a bug!
    commit_swapped_parents=$(git commit-tree -p "HEAD^2" -p "HEAD^1" -m "GitButler Workspace Commit" "HEAD^{tree}")
    echo "${commit_swapped_parents}" >.git/refs/heads/gitbutler/workspace

    git checkout --orphan disjoint-target-tracking
    commit "disjoint remote target"

    setup_remote_tracking disjoint-target-tracking main 'move'
    git checkout gitbutler/workspace
  )

  git init two-dependent-branches-with-interesting-remote-setup
  (cd two-dependent-branches-with-interesting-remote-setup
    commit init
    setup_target_to_match_main

    git checkout -b integrated
      commit "integrated in target"
      commit "other integrated"

    git checkout -b soon-A-remote
      commit "shared by name"
    setup_remote_tracking soon-A-remote A "move"

    git checkout -b soon-main-remote integrated
      commit "another unrelated"

    git checkout -b A
      commit "shared by name" --allow-empty

    setup_remote_tracking soon-main-remote main "move"
    create_workspace_commit_once A
  )

  git init no-target-without-ws-commit
  (cd no-target-without-ws-commit
    commit init
    git checkout -b A
      commit A1
      commit A2
    git branch gitbutler/workspace
    git checkout -b soon-A-remote
      commit A-remote
      setup_remote_tracking soon-A-remote A "move"
    git checkout gitbutler/workspace
  )

  git init no-target-without-ws-commit-ambiguous
  (cd no-target-without-ws-commit-ambiguous
    commit init
    git checkout -b A
      commit A1
      commit A2
    git branch gitbutler/workspace
    git branch B
    git checkout -b soon-A-remote
      commit A-remote
      setup_remote_tracking soon-A-remote A "move"
    git checkout gitbutler/workspace
  )

  cp -R no-target-without-ws-commit-ambiguous no-target-without-ws-commit-ambiguous-with-remotes
  (cd no-target-without-ws-commit-ambiguous-with-remotes
    add_main_remote_setup
    remote_tracking_caught_up A
    remote_tracking_caught_up B
  )

  git init no-target-with-ws-commit
  (cd no-target-with-ws-commit
    commit init
    git checkout -b A
      commit A1
      commit A2
    git checkout -b soon-A-remote
      commit A-remote
      setup_remote_tracking soon-A-remote A "move"

    git checkout A
    create_workspace_commit_once A
  )

  git init ws-commit-pushed-to-target
  (cd ws-commit-pushed-to-target
    commit init
    git checkout -b A
      commit A1
    create_workspace_commit_once A
    git checkout -b soon-main-remote
      setup_remote_tracking soon-main-remote main "move"

    git checkout gitbutler/workspace
  )

  git init no-ws-no-target-commit-with-managed-ref
  (cd no-ws-no-target-commit-with-managed-ref
    commit init
    git checkout -b A
      commit A1
    git checkout -b gitbutler/workspace
      commit unmanaged
  )

  git init one-stacks-many-refs
  (cd one-stacks-many-refs
    commit init && setup_target_to_match_main
    for name in A B C;  do
      git branch "$name"
    done
    git checkout -b S1
      commit 1
        git branch D
        git branch E
      commit 2
        git branch F
        git branch G

    create_workspace_commit_once S1
  )

  git init multiple-dependent-branches-per-stack-without-ws-commit
  (cd multiple-dependent-branches-per-stack-without-ws-commit
    git commit -m "init" --allow-empty
    setup_target_to_match_main

    git branch lane-segment-01
    git branch lane-segment-02

    git branch lane-2
    git branch lane-2-segment-01
    git branch lane-2-segment-02

    git checkout -b lane
    commit "change"

    git checkout -b gitbutler/workspace
  )
  git init "two-dependent-branches-first-rebased-and-merged"
  (cd "two-dependent-branches-first-rebased-and-merged"
    echo init>file && git add file && git commit -m "init"
    git checkout -b A && echo A >>file && git commit -am "A"
    git checkout -b B && echo B >>file && git commit -am "B"
    create_workspace_commit_once B
    git checkout -b soon-origin-main main
      tick
      git cherry-pick A

    git checkout gitbutler/workspace
    setup_remote_tracking soon-origin-main main "move"

    add_main_remote_setup
    cp .git/refs/remotes/origin/main .git/refs/remotes/origin/A
  )

  git init "two-dependent-branches-rebased-with-remotes-merge-one-local"
  (cd "two-dependent-branches-rebased-with-remotes-merge-one-local"
    echo init>file && git add file && git commit -m "init"
    git checkout -b A && echo A >>file && git commit -am "A"
    git checkout -b B && echo B >>file && git commit -am "B"
    create_workspace_commit_once B
    git checkout -b soon-origin-A main
      tick
      git cherry-pick A
      git checkout -b soon-origin-B
      git cherry-pick B
    git checkout -b soon-origin-main main
      git merge --no-ff A
    git checkout gitbutler/workspace

    setup_remote_tracking soon-origin-A A "move"
    setup_remote_tracking soon-origin-B B "move"
    setup_remote_tracking soon-origin-main main "move"

    add_main_remote_setup
  )

  git init "two-dependent-branches-rebased-with-remotes-squash-merge-one-remote"
  (cd "two-dependent-branches-rebased-with-remotes-squash-merge-one-remote"
    echo init>file && git add file && git commit -m "init"
    git checkout -b A && echo A >>file && git commit -am "A"
    git checkout -b B && echo B >>file && git commit -am "B"
    git checkout -b C && echo C >>file && git commit -am "C"
    git checkout -b D && echo D >>file && git commit -am "D"

    git checkout main
      tick
      # easy squash-merge simulation of only A
      git cherry-pick A
      setup_target_to_match_main
    git checkout -b rebased-D
      git cherry-pick B
      git cherry-pick C
      git cherry-pick D

      # setup free-standing remotes that were previously pushed.
      # replace local branches as they don't matter there.
      setup_remote_tracking A A "move"
      setup_remote_tracking B B "move"
      setup_remote_tracking C C "move"
      setup_remote_tracking D D "move"

      # get our rebased tip back
      git branch -m D

    create_workspace_commit_once D
  )

  git init "two-dependent-branches-rebased-with-remotes-squash-merge-one-remote-ambiguous"
  (cd "two-dependent-branches-rebased-with-remotes-squash-merge-one-remote-ambiguous"
    echo init>file && git add file && git commit -m "init"
    git checkout -b A && echo A >>file && git commit -am "A"
    git branch B
    git branch C
    git checkout -b D && echo D >>file && git commit -am "D"

    git checkout main
      tick
      # easy squash-merge simulation of only A
      git cherry-pick A
      setup_target_to_match_main
    git checkout -b rebased-D
      git cherry-pick D

      # setup free-standing remotes that were previously pushed.
      # replace local branches as they don't matter there.
      setup_remote_tracking A A "move"
      setup_remote_tracking B B "move"
      setup_remote_tracking C C "move"
      setup_remote_tracking D D "move"

      # get our rebased tip back
      git branch -m D

    create_workspace_commit_once D
  )

  git init special-branches
  (cd special-branches
    commit init
      git branch gitbutler/target
    commit middle
      git branch gitbutler/edit
    commit top
    create_workspace_commit_once main
  )

  git init special-branches-edgecase
  (cd special-branches-edgecase
    commit init
    commit M1
    git branch gitbutler/target
    setup_remote_tracking "gitbutler/target"
    commit M2
    setup_target_to_match_main
    git checkout -b A
    commit middle
      git branch gitbutler/edit
    commit top
    create_workspace_commit_once A
  )

  git init branches-ahead-of-workspace
  (cd branches-ahead-of-workspace
    commit init
    add_main_remote_setup

    git checkout -b A
      git branch B
      git branch C
      git branch D
      commit A1
      git branch A-middle
      commit A2
    git checkout B
      commit B1
      commit B2
      git branch B-middle
      remote_tracking_caught_up B-middle
      commit B3
    git checkout C
      commit C1
      git branch C-bottom
      commit C2
    git checkout D
      commit D1
      git branch new-name-for-D

    git checkout main
      git merge --no-ff A
      git merge --no-ff B-middle
      remote_tracking_caught_up main

    git checkout A
    create_workspace_commit_once A B C D

    git checkout A
      commit A2-outside
    git checkout A-middle
      commit A1-outside
      remote_tracking_caught_up A-middle
    git checkout B-middle
      commit B2-outside
      git branch intermediate-branch
      commit B3-outside
    git checkout C-bottom
      commit C1-outside
      git checkout -b tmp @~1
        tick
        commit C1-outside2
      git checkout C-bottom
      git merge --no-ff tmp -m "C2 merge commit"
    git checkout D
      commit D2-outside
    git checkout gitbutler/workspace
  )

  git init advanced-workspace-ref
  (cd advanced-workspace-ref
    commit M1
    commit M2
    setup_target_to_match_main
    git checkout -b A
      git branch B
      commit A1
    git checkout B
      commit B1

    create_workspace_commit_once B A
    commit on-top1
    git checkout -b branch-on-top
      commit on-top-sibling
    git checkout gitbutler/workspace
    git merge --no-ff branch-on-top -m "on-top2-merge"
    commit on-top3
    git branch intermediate-ref
    commit on-top4
  )

  git init advanced-workspace-ref-and-single-stack
  (cd advanced-workspace-ref-and-single-stack
    commit M1
    commit M2
    setup_target_to_match_main
    git checkout -b A
      commit A1
    create_workspace_commit_once A
    commit on-top1
    git checkout -b branch-on-top
      commit on-top-sibling
    git checkout gitbutler/workspace
    git merge --no-ff branch-on-top -m "on-top2-merge"
    commit on-top3
    git branch intermediate-ref
    commit on-top4

    git checkout gitbutler/workspace
  )

  git init two-branches-one-below-base
  (cd two-branches-one-below-base
    commit M1
    commit M2
    git checkout -b A
      commit A1
    git checkout main
      tick
      commit M3
      # important to have a clear target right below B,
      # so A is below that.
      git branch B
      commit M4
      setup_target_to_match_main
    git checkout B
      commit B1
    create_workspace_commit_once B A
  )

  git init two-branches-one-above-base
  (cd two-branches-one-above-base
    commit M1
    commit M2
    git branch B
    commit M3
    setup_target_to_match_main
    git checkout -b A
      commit A1
    git checkout B
      tick
      commit B1
    create_workspace_commit_once B A
  )

  git init dependent-branch-on-base
  (cd dependent-branch-on-base
    commit M1
    setup_target_to_match_main
    git branch B
    git branch below-below-A
    git branch below-A
    git branch below-B
    git branch below-below-B
    git branch C
    git branch below-C
    git branch below-below-C
    git checkout -b A
      commit A1
    git checkout C
      commit C1
      git branch C1-1
      git branch C1-2
      git branch C1-3
      commit C2
      git branch C2-1
      git branch C2-2
      git branch C2-3
    create_workspace_commit_aggressively C B A
  )

  mkdir edit-commit
  (cd edit-commit
    git init simple
    (cd simple
      commit init
      setup_target_to_match_main
      git checkout -b A
        commit A1
        git branch gitbutler/edit
        commit A2
      create_workspace_commit_once A
    )
  )

  git init local-target-ahead-and-on-stack-tip
  (cd local-target-ahead-and-on-stack-tip
    commit init
    setup_target_to_match_main
    commit A
    git checkout -b A
    create_workspace_commit_once A
  )

  git init unapplied-branch-on-base
  (cd unapplied-branch-on-base
    commit init
    git branch unapplied
    setup_target_to_match_main
    create_workspace_commit_once main
  )

  git init remote-far-in-ancestry
  (cd remote-far-in-ancestry
    commit M1
    git checkout -b soon-A-remote
      commit R2
      commit R3
      setup_remote_tracking soon-A-remote A move
    git checkout main
    commit M2
    commit M3
    commit M4
    commit M5
    commit M6
    commit M7
    commit M8
    commit M9
    commit M10
    commit M11
    commit M12
    setup_target_to_match_main
    git checkout -b A
    commit A1
    commit A2
    commit A3
    create_workspace_commit_once A
  )


  git init no-ws-ref-no-ws-commit-two-branches
  (cd no-ws-ref-no-ws-commit-two-branches
    commit M1
    commit M2

    git branch A
    git branch B

    create_workspace_commit_once A B
  )

  git init main-with-remote-and-workspace-ref
  (cd main-with-remote-and-workspace-ref
    commit M1
    commit on-remote-only
    setup_target_to_match_main
    git reset --hard @~1
    git branch gitbutler/workspace
  )
)

