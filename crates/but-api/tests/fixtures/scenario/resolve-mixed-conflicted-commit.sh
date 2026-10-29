#!/usr/bin/env bash

set -eu -o pipefail

git init
echo "A conflicted commit mixing a hunk-addressable conflict with a binary one" >.git/description

git config user.name GitButler
git config user.email gitbutler@example.com

echo unrelated >file
git add . && git commit -m "init"

mkdir -p .git/refs/remotes/origin
cp .git/refs/heads/main .git/refs/remotes/origin/main

cat <<EOF >>.git/config
[remote "origin"]
	url = ./fake/local/path/which-is-fine-as-we-dont-fetch-or-push
	fetch = +refs/heads/*:refs/remotes/origin/*
EOF

# A conflicted commit as GitButler would write it after a rebase: the merge
# inputs are kept as trees, the commit tree is auto-resolved favoring "ours",
# and the conflict is recorded in the message trailer plus the legacy header.
#
# The content conflicts: base, ours (the new base), and theirs (the commit's
# own version) each differ at "line two" and at "line six" of "conflict",
# far enough apart to form two separate conflict hunks.
unrelated_blob=$(git rev-parse HEAD:file)
base_blob=$(printf "line one\nline two\nline three\nline four\nline five\nline six\nline seven\n" | git hash-object -wt blob --stdin)
ours_blob=$(printf "line one\nline two changed by the new base\nline three\nline four\nline five\nline six changed by the new base\nline seven\n" | git hash-object -wt blob --stdin)
theirs_blob=$(printf "line one\nline two changed by this commit\nline three\nline four\nline five\nline six changed by this commit\nline seven\n" | git hash-object -wt blob --stdin)
# A binary file conflicting on both sides too. It has no marker representation,
# so it can only ever be resolved in edit mode — the point of the fixture is
# that its presence must not hide the text conflicts above.
base_bin=$(printf 'bin\xff\xfe base\n' | git hash-object -wt blob --stdin)
ours_bin=$(printf 'bin\xff\xfe ours\n' | git hash-object -wt blob --stdin)
theirs_bin=$(printf 'bin\xff\xfe theirs\n' | git hash-object -wt blob --stdin)

conflict_files_blob=$(git hash-object -wt blob --stdin <<EOF
ancestorEntries = [ "conflict", "binary" ]
ourEntries = [ "conflict", "binary" ]
theirEntries = [ "conflict", "binary" ]
EOF
)

# The commit the conflicted one was rebased onto: its tree is the "ours" side,
# exactly like the rebase engine leaves it.
git read-tree --empty
git update-index --add --cacheinfo 100644 "$unrelated_blob" "file"
git update-index --add --cacheinfo 100644 "$ours_blob" "conflict"
git update-index --add --cacheinfo 100644 "$ours_bin" "binary"
new_base_tree=$(git write-tree)
new_base_commit=$(git commit-tree "$new_base_tree" -p "$(git rev-parse HEAD)" -m "new base")

git read-tree --empty
git update-index --add --cacheinfo 100644 "$unrelated_blob" ".auto-resolution/file"
git update-index --add --cacheinfo 100644 "$ours_blob" ".auto-resolution/conflict"
git update-index --add --cacheinfo 100644 "$ours_bin" ".auto-resolution/binary"
git update-index --add --cacheinfo 100644 "$unrelated_blob" ".conflict-base-0/file"
git update-index --add --cacheinfo 100644 "$base_blob" ".conflict-base-0/conflict"
git update-index --add --cacheinfo 100644 "$base_bin" ".conflict-base-0/binary"
git update-index --add --cacheinfo 100644 "$conflict_files_blob" ".conflict-files"
git update-index --add --cacheinfo 100644 "$unrelated_blob" ".conflict-side-0/file"
git update-index --add --cacheinfo 100644 "$ours_blob" ".conflict-side-0/conflict"
git update-index --add --cacheinfo 100644 "$ours_bin" ".conflict-side-0/binary"
git update-index --add --cacheinfo 100644 "$unrelated_blob" ".conflict-side-1/file"
git update-index --add --cacheinfo 100644 "$theirs_blob" ".conflict-side-1/conflict"
git update-index --add --cacheinfo 100644 "$theirs_bin" ".conflict-side-1/binary"
conflict_tree=$(git write-tree)

conflict_commit=$(git hash-object -wt commit --stdin <<EOF
tree $conflict_tree
parent $new_base_commit
author GitButler <gitbutler@example.com> 1730625617 +0100
committer GitButler <gitbutler@example.com> 1730625617 +0100
gitbutler-headers-version 2
change-id 00000000-0000-0000-0000-000000000001
gitbutler-conflicted 1

[conflict] Change line two

GitButler-Conflict: fixture marker

EOF
)
git tag conflicted "$conflict_commit"

# A normal descendant of the conflicted commit. Its tree is built on the
# conflicted commit's auto-resolution, like the rebase engine would do.
later_blob=$(printf "descendant\n" | git hash-object -wt blob --stdin)
git read-tree --empty
git update-index --add --cacheinfo 100644 "$unrelated_blob" "file"
git update-index --add --cacheinfo 100644 "$ours_blob" "conflict"
git update-index --add --cacheinfo 100644 "$ours_bin" "binary"
git update-index --add --cacheinfo 100644 "$later_blob" "later"
descendant_tree=$(git write-tree)
descendant_commit=$(git commit-tree "$descendant_tree" -p "$conflict_commit" -m "descendant")
git update-ref refs/heads/branchy "$descendant_commit"

git symbolic-ref HEAD refs/heads/branchy
git reset --hard >/dev/null
