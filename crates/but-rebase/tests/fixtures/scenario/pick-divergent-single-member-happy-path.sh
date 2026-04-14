#!/bin/bash

set -eu -o pipefail

git init

cat <<'EOF' >divergent.txt
1
2
3
4
EOF
git add divergent.txt
git commit -m "divergence-base"
git update-ref refs/heads/ancestor "$(git rev-parse HEAD)"

git branch local
git checkout local
cat <<'EOF' >divergent.txt
1
local-2
3
4
EOF
git add divergent.txt
git commit -m "local-change-line-two"
git update-ref refs/heads/local "$(git rev-parse HEAD)"

git checkout main
cat <<'EOF' >divergent.txt
1
2
3
remote-4
EOF
git add divergent.txt
git commit -m "remote-change-line-four"
git update-ref refs/heads/remote "$(git rev-parse HEAD)"

echo "tip" >tip.txt
git add tip.txt
git commit -m "tip-after-remote"
