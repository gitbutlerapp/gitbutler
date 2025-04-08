#!/usr/bin/env bash

### Description
# Various files, of which two are in the worktree (with changes) and in the index (with changes).
# Another pair of files was deleted, both in the worktree and in the index.
# Lastly, each of these have specific worktree modifications in multiple hunks, and a plain file was made
# executable in the index, and another rename destination was made executable, too.
# Lastly, a simple file was executable, which isn't executable anymore.
set -eu -o pipefail

git init
seq 5 18 >file && chmod +x file
seq 5 18 >file-in-index
seq 5 18 >file-to-be-renamed
seq 5 18 >file-to-be-renamed-in-index
git add . && git commit -m "init"

cat <<EOF >file
1
2
3
4
5
6-7
8
9
ten
eleven
12
20
21
22
15
16
EOF

chmod -x file

cp file file-in-index && \
  chmod +x file-in-index && \
  git add file-in-index


seq 2 18 >file-to-be-renamed && \
  mv file-to-be-renamed file-renamed && \
  chmod +x file-renamed
cp file file-to-be-renamed-in-index && \
  git mv file-to-be-renamed-in-index file-renamed-in-index


