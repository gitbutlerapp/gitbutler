#!/usr/bin/env bash

### Description
# A directory with a file, an executable and a symlink is moved into a sibling file.
set -eu -o pipefail

git init
mkdir -p a/b
(cd a/b
  seq 5 8 >file
  seq 1 3 >executable && chmod +x executable
  ln -s nonexisting-target link
)
seq 10 13 >a/sibling

git add . && git commit -m "init"

rm a/sibling
mv a/b a/sibling


