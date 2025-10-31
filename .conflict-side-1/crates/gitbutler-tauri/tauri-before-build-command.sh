#!/usr/bin/env bash
set -euo pipefail

if [ "${CI:-}" = "true" ]; then
  echo "CI environment detected, expecting frontend build to be downloaded."
else
  fe_mode=${1:?First argument must be the mode to build the frontend with}
  echo "Assuming local invocation, building frontend in $fe_mode mode"
  pnpm build:desktop -- --mode "$fe_mode"
fi

cargo build --release -p gitbutler-git
bash ./crates/gitbutler-tauri/inject-git-binaries.sh
