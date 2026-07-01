#!/usr/bin/env bash

set -eu -o pipefail
source "${BASH_SOURCE[0]%/*}/shared.sh"

function commit-file() {
  local name="${1:?First argument is the filename}"
  local content=${2:-$1}
  echo $content >$name && git add $name && git commit -m "add $content"
}

### Description
# A couple of independent heads where some merge cleanly, and some don't, with `conflict-hero` conflicting with two different branches.
# A workspace ref is present to make it easier to discover the branches of interest in the graph.
git init
commit M
git branch gitbutler/workspace

for filename in A B C; do
  git checkout -b clean-$filename main
    commit-file $filename
done

git checkout -b conflict-F1 main
  commit-file F1

git checkout -b conflict-F2 main
  commit-file F2

git checkout -b conflict-hero main
  commit-file F1 conflicting-F1
  commit-file F2 conflicting-F2
