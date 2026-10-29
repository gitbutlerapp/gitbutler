#!/usr/bin/env bash

# Two branches with different merge bases with the target.
#
#   A-change (A)    B-change (B)
#       |               |
#     base ─── M1 ─── M2 (main)
#       ↑               ↑
#  merge_base(A)    merge_base(B)
#
# The graph walk uses the combined merge base of all stack heads ("base"),
# so B's stack will include M1 and M2 below B's own merge base (M2).
# Pruning should truncate those extra commits from B's display.

set -eu -o pipefail

source "${BASH_SOURCE[0]%/*}/shared.sh"

git-init-frozen

echo base > file.txt
git add file.txt
git commit -m "base"
git update-ref refs/heads/base HEAD

git checkout -b A

echo change-A > file-a.txt
git add file-a.txt
git commit -m "A-change"

git checkout main
echo m1 > m1.txt && git add m1.txt && git commit -m "M1"
echo m2 > m2.txt && git add m2.txt && git commit -m "M2"

git checkout -b B

echo change-B > file-b.txt
git add file-b.txt
git commit -m "B-change"

git checkout main
setup_target_to_match_main

git checkout B
create_workspace_commit_once A B
