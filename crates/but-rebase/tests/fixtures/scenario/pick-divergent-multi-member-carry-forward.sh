#!/bin/bash

set -eu -o pipefail

git init

cat <<'EOF' >file.txt
1
2
3
4
5
6
7
8
EOF
git add file.txt
git commit -m "divergence-base"
git update-ref refs/heads/ancestor "$(git rev-parse HEAD)"

git branch local
git checkout local
cat <<'EOF' >file.txt
1
2
local-top
3
4
5
6
7
8
EOF
git add file.txt
git commit -m "local-top"
git update-ref refs/heads/local-top "$(git rev-parse HEAD)"

cat <<'EOF' >file.txt
1
2
local-top
3
4
5
6
7
8
local-bottom
EOF
git add file.txt
git commit -m "local-bottom"
git update-ref refs/heads/local-bottom "$(git rev-parse HEAD)"

git checkout main
cat <<'EOF' >file.txt
1
2
remote-1
3
4
5
6
7
8
EOF
git add file.txt
git commit -m "remote-1"
git update-ref refs/heads/remote-one "$(git rev-parse HEAD)"

cat <<'EOF' >file.txt
1
2
remote-1
3
4
remote-2
5
6
7
8
EOF
git add file.txt
git commit -m "remote-2"
git update-ref refs/heads/remote-two "$(git rev-parse HEAD)"

echo "remote-extra" >extra.txt
git add extra.txt
git commit -m "remote-3"
git update-ref refs/heads/remote-three "$(git rev-parse HEAD)"
