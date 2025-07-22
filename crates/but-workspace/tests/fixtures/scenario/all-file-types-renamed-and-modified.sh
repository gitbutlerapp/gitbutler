#!/usr/bin/env bash

### Description
# A commit with an executable, a normal file, a symlink and an untracked fifo.
# Then each item gets renamed in the worktree.

function add_change_id_and_reset_head() {
   local change_id="00000000-0000-0000-0000-000000003333"

   # Insert the Change-ID header lines after the committer line.
   git cat-file -p "${1:?first argument is the commit to add a changeid to}" \
   | awk -v cid="$change_id" '
     BEGIN { injected = 0 }
     /^$/ && !injected {
       print "gitbutler-headers-version 2"
       print "gitbutler-change-id " cid
       print ""
       injected = 1
       next
     }
     { print }
     ' \
   | git hash-object -wt commit --stdin >.git/refs/heads/main
}
set -eu -o pipefail

git init
seq 5 8 >file
seq 1 3 >executable && chmod +x executable
ln -s nonexisting-target link
mkfifo fifo-should-be-ignored

git add . && git commit -m "init"
add_change_id_and_reset_head "$(git rev-parse HEAD)"

seq 5 10 >file
seq 1 5 >executable
mv file file-renamed
mv executable executable-renamed
chmod +x executable-renamed

rm link
ln -s other-nonexisting-target link-renamed

