#!/usr/bin/env bash
set -eu -o pipefail

git init intent-to-add
(cd intent-to-add
  git commit -m "init" --allow-empty
  echo -n content >to-be-added
  git add --intent-to-add to-be-added
)

git init module;
(cd module
  mkdir dir
  touch a dir/b
  git add . && git commit -m "init"
)

git init with-submodule-in-index
(cd with-submodule-in-index
  git commit -m "init" --allow-empty
  git submodule add ../module sm
)

git init with-submodule-new-commit
(cd with-submodule-new-commit
  git commit -m "init" --allow-empty
  git submodule add ../module sm
  git commit -m "add submodule"
  (cd sm
    echo -n change >>a && git commit -am "change file in submodule"
  )
)
