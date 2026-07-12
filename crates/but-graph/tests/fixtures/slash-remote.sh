#!/usr/bin/env bash

### General Description

# A workspace whose target remote has a slash in its name (`special/origin`) —
# remote-name extraction must never assume slash-free remote names.
source "${BASH_SOURCE[0]%/*}/shared.sh"

git init slash-remote
(cd slash-remote
    commit init
    git checkout -b A
      commit A1
    git checkout main
      commit M2
  create_workspace_commit_once main A

  git checkout -b soon-remote-main main~1
    commit RM1
  git checkout gitbutler/workspace

  cat <<EOF >>.git/config
[remote "special/origin"]
	url = ./fake/local/path/which-is-fine-as-we-dont-fetch-or-push
	fetch = +refs/heads/*:refs/remotes/special/origin/*

[branch "main"]
	remote = special/origin
	merge = refs/heads/main
EOF

  mkdir -p .git/refs/remotes/special/origin
  mv .git/refs/heads/soon-remote-main .git/refs/remotes/special/origin/main
)
