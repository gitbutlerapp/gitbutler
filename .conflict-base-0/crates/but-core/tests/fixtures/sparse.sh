#!/bin/bash

set -eu -o pipefail

git init -q non-cone
(cd non-cone
  touch a b
  mkdir c1
  (cd c1 && touch a b && mkdir c2 && cd c2 && touch a b)
  (cd c1 && mkdir c3 && cd c3 && touch a b)
  mkdir d
  (cd d && touch a b && mkdir c4 && cd c4 && touch a b c5)

  git add .
  git commit -m "init"

  git sparse-checkout set c1/c2 --sparse-index
)

