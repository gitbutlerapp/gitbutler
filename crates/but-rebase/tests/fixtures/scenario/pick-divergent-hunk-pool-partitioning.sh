#!/bin/bash

set -eu -o pipefail

git init

cat <<'EOF' >file.txt
1
2
3
4
5
EOF
git add file.txt
git commit -m "divergence-base"
git update-ref refs/heads/base "$(git rev-parse HEAD)"

git branch local
git checkout local
cat <<'EOF' >file.txt
1
2
LOCAL
3
4
5
EOF
git add file.txt
git commit -m "local-insert"
git update-ref refs/heads/local "$(git rev-parse HEAD)"

git checkout main
cat <<'EOF' >file.txt
1
2
R1
3
4
5
EOF
git add file.txt
git commit -m "remote-1"
git update-ref refs/heads/remote-one "$(git rev-parse HEAD)"

git checkout -b carried
cat <<'EOF' >file.txt
1
2
R1
LOCAL
3
4
5
EOF
git add file.txt
git commit -m "carried-local"
git update-ref refs/heads/carried-local "$(git rev-parse HEAD)"

git checkout main
cat <<'EOF' >file.txt
1
2
R1
R2
3
4
5
EOF
git add file.txt
git commit -m "remote-2"
git update-ref refs/heads/remote-two "$(git rev-parse HEAD)"
