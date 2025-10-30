#!/bin/bash

set -eu -o pipefail

# A repository with a normal and an artificial conflicting commit
git init normal-and-artificial
(cd normal-and-artificial
  echo content >file && git add . && git commit -m "init"
  git tag normal

  conflict_files_id=$(git hash-object -wt blob --stdin <<EOF
ancestorEntries = [ "a-one", "a-two" ]
ourEntries = [ "o-one", "o-two" ]
theirEntries = [ "t-one", "t-two" ]
EOF
  )

  empty_blob=$(git hash-object -wt blob /dev/null)
  git update-index --index-info <<EOF
100644 blob $empty_blob	.auto-resolution/file
100644 blob $empty_blob	.conflict-base-0/file
100644 blob $conflict_files_id	.conflict-files
100644 blob $empty_blob	.conflict-side-0/file
100644 blob $empty_blob	.conflict-side-1/file
100644 blob $empty_blob	README.txt
EOF

  conflict_tree=$(git write-tree)

  conflict_commit=$(git hash-object -wt commit --stdin <<EOF
tree $conflict_tree
parent $(git rev-parse HEAD)
author GitButler <gitbutler@gitbutler.com> 1730625617 +0100
committer Sebastian Thiel <sebastian.thiel@icloud.com> 1736343261 +0100
gitbutler-headers-version 2
gitbutler-change-id 0f74c342-1cd3-4408-b965-6c2dfac89857
gitbutler-conflicted 2

GitButler WIP Commit


EOF
  )
    echo $conflict_commit >.git/HEAD
    git tag conflicted

)
