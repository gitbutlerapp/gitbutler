#!/usr/bin/env bash

source "${BASH_SOURCE[0]%/*}/../shared.sh"

set -eu -o pipefail

git init
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
