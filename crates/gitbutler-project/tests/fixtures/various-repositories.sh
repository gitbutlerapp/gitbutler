#!/usr/bin/env bash
set -eu -o pipefail

git init simple
(cd simple
  >file && git add file && git commit -m "init"
)

git clone simple submodule

git clone simple with-submodule
(cd with-submodule
  git submodule add ../submodule
  git commit -m "add submodule"
)
