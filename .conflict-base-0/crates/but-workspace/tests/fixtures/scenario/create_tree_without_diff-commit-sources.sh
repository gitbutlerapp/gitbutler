#!/usr/bin/env bash

source "${BASH_SOURCE[0]%/*}/shared.sh"

git init
echo "create_tree_without_diff commit-source scenarios" >.git/description

cat >regular-change.txt <<'EOF'
base-1
base-2
EOF
git add regular-change.txt
git commit -m "regular parent"
git tag regular-parent

cat >>regular-change.txt <<'EOF'
keep-1
drop-1
drop-2
keep-2
EOF
git add regular-change.txt
git commit -m "regular source modifies existing file"
git tag regular-source

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
git reset --soft $conflict_commit
git tag regular-then-conflicted-source

cat >file <<'EOF'
keep-a
drop-a
drop-b
keep-b
EOF
git add -A
git commit -m "regular source modifies file after conflicted parent"
git tag conflicted-then-regular-source
