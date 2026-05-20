#!/usr/bin/env bash
set -euo pipefail

if [ "${CI:-}" = "true" ]; then
  echo "CI environment detected, expecting frontend build to be downloaded."
else
  fe_mode=${1:?First argument must be the mode to build the frontend with}
  echo "Assuming local invocation, building frontend in $fe_mode mode"
  pnpm build:desktop -- --mode "$fe_mode"
fi

set -x
cargo build --release -p gitbutler-git
if [ "${OS:-}" == "windows" ] || [ "${OS:-}" == "linux" ]; then
  # NOTE: Should run either if the builtin-but feature is *not* selected in `release.sh` (case for Windows), OR if we
  # need the standalone CLI for separate publishing (case for Linux)
  cargo build --release -p but
fi
bash ./crates/gitbutler-tauri/inject-git-binaries.sh
