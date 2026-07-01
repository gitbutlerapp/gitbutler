#!/usr/bin/env bash
set -eu -o pipefail

git init -b master submodule-source
(cd submodule-source
  # These tests open the fixture with Controller::from_path, which eventually
  # opens a non-isolated gix repository. Keep identity in the fixture so the
  # tests do not depend on the developer's global Git config.
  git config commit.gpgsign false
  git config user.name gitbutler-test
  git config user.email gitbutler-test@example.com
  printf "submodule content" >file.txt
  git add file.txt
  git commit -m "Initial submodule commit"
)

git init -b master local
(cd local
  # These tests open the fixture with Controller::from_path, which eventually
  # opens a non-isolated gix repository. Keep identity in the fixture so the
  # tests do not depend on the developer's global Git config.
  git config commit.gpgsign false
  git config user.name gitbutler-test
  git config user.email gitbutler-test@example.com
  git config gitbutler.storagePath gitbutler
  git commit --allow-empty -m "Initial commit"
)

git init --bare remote
(cd local
  remote_path="$(cd ../remote && pwd)"
  git remote add origin "$remote_path"
  git push origin master
  git fetch origin '+refs/heads/*:refs/remotes/origin/*'
  git -c protocol.file.allow=always submodule add ../submodule-source submodule
)
