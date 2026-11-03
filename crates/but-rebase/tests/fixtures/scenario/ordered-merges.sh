#!/bin/bash

set -eu -o pipefail
source "${BASH_SOURCE[0]%/*}/shared.sh"

function commit() {
  local letters=$1 name=$2
  for ((i = 0; i < ${#letters}; i++)); do
    printf '%s\n-\n-\n-\n' "${letters:i:1}"
  done >file
  tick
  git add file && git commit -qm "$name" && git tag "$name"
}

function merge() {
  local name=$1
  shift
  tick
  git merge -q --no-ff -m "$name" "$@" && git tag "$name"
}

function empty() {
  tick
  git commit -q --allow-empty -m "$1" && git tag "$1"
}

git init nested
(cd nested
  commit aaa c1
  commit baa c2
  commit cba p1
  git checkout -q --detach c2
  commit baax p3
  git checkout -q --detach c1
  commit aab p2
)

git init reverted
(cd reverted
  commit aaa c1
  commit baa c2
  commit aca p1
  git checkout -q --detach c2
  commit baax p3
  git checkout -q --detach c1
  commit aab p2
)

git init five-leaves
(cd five-leaves
  empty Z
  empty A
  empty c1
  git checkout -q --detach A
  empty c2
  git checkout -q --detach A
  empty c3
  git checkout -q --detach Z
  empty B
  empty c4
  git checkout -q --detach B
  empty c5
)

git init criss-cross
(cd criss-cross
  empty Z
  empty X1
  empty X
  git checkout -q --detach Z
  empty Y
  merge a X
  git checkout -q --detach X
  merge b Y
)

git init unrelated
(cd unrelated
  empty r1
  git checkout -q --orphan other
  empty r2
)

git init merged-root
(cd merged-root
  empty m1
  empty m2
  git checkout -q --orphan subtree
  empty s1
  empty s2
  git checkout -q --detach m2
  merge a --allow-unrelated-histories s2
  git checkout -q --detach m2
  empty b
)

git init newer-ancestor
(cd newer-ancestor
  GIT_COMMITTER_DATE="1900000000 +0000" git commit -q --allow-empty -m base && git tag base
  empty B
)

git init shallow-source
(cd shallow-source
  empty R
  GIT_COMMITTER_DATE="1900000000 +0000" git commit -q --allow-empty -m S
  git branch b
  git checkout -q -b a
  empty a
  git checkout -q b
  empty b
)
git clone -q --shallow-exclude R --no-single-branch --no-tags "file://$PWD/shallow-source" shallow
(cd shallow
  git tag a origin/a
  git tag b origin/b
  git tag S origin/a~1
)
