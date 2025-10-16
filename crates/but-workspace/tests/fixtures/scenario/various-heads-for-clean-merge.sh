#!/usr/bin/env bash

set -eu -o pipefail
source "${BASH_SOURCE[0]%/*}/shared.sh"

function commit-file() {
  local name="${1:?First argument is the filename}"
  echo $name >$name && git add $name && git commit -m "add $name"
}

### Description
# A couple of independent heads which merge cleanly, each adding a file.
# A workspace ref is present to make it easier to discover the branches of interest in the graph.
git init
commit M
git branch gitbutler/workspace

for filename in A B C D; do
  git checkout -b add-$filename main
    commit-file $filename
done
