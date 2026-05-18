#!/usr/bin/env bash
set -euo pipefail

run_package_script() {
	local script_name
	script_name="$1"
	shift

	case "${JS_PACKAGE_MANAGER:-pnpm}" in
		bun)
			bun run "$script_name" "$@"
			;;
		pnpm)
			pnpm "$script_name" "$@"
			;;
		*)
			echo "unsupported JS_PACKAGE_MANAGER: ${JS_PACKAGE_MANAGER:-}" >&2
			exit 1
			;;
	esac
}

if [ "${CI:-}" = "true" ]; then
  echo "CI environment detected, expecting frontend build to be downloaded."
else
  fe_mode=${1:?First argument must be the mode to build the frontend with}
  echo "Assuming local invocation, building frontend in $fe_mode mode"
  run_package_script build:desktop -- --mode "$fe_mode"
fi

set -x
cargo build --release -p gitbutler-git
if [ "${OS:-}" == "windows" ] || [ "${OS:-}" == "linux" ]; then
  # NOTE: Should run either if the builtin-but feature is *not* selected in `release.sh` (case for Windows), OR if we
  # need the standalone CLI for separate publishing (case for Linux)
  # remote brings in but-server and the embedded frontend; excluded from release builds.
  BUT_FEATURES=""
  if [ "${CHANNEL:-}" != "release" ]; then
    BUT_FEATURES="--features irc,remote"
    # Build a web-targeted frontend for `but remote` into a separate directory
    # so it doesn't clobber the Tauri-targeted build in apps/desktop/build/.
    # In CI, the web frontend is pre-built and downloaded as an artifact.
    if [ "${CI:-}" != "true" ]; then
      SVELTEKIT_OUT_DIR=embedded-frontend \
        VITE_BUILD_TARGET=web VITE_BUTLER_API_BASE_URL=/api VITE_EMBEDDED_BUILD=1 \
        run_package_script build:desktop
    fi
  fi
  # shellcheck disable=SC2086
  cargo build --release -p but $BUT_FEATURES
fi
bash ./crates/gitbutler-tauri/inject-git-binaries.sh
