#!/usr/bin/env bash

### Description
# A single tracked file, modified in the workspace to have:
# - added lines at the beginning
# - deleted lines at the end
# - modified lines, added lines, and deleted lines directly after one another in the middle.
set -eu -o pipefail

git init
seq 5 18 >file
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

cp file file-in-index && git add file-in-index


seq 2 18 >file-to-be-renamed && mv file-to-be-renamed file-renamed
cp file file-to-be-renamed-in-index && git mv file-to-be-renamed-in-index file-renamed-in-index


