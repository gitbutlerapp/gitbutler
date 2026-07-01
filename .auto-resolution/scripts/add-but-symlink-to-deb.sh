#!/usr/bin/env bash
#
# Repackages a .deb file to include a 'but' symlink pointing to 'gitbutler-tauri'
# in /usr/bin/. The symlink is added as a proper tracked file in the package so
# dpkg knows about it.
#
# We need this as Tauri currently resolves symlinks when bundling, so we can't
# easily add in a symlink with Tauri-native packaging short of using a
# postinstall script. That solution is itself not great as then the package
# manager can't track the symlink, so it's better for everyone to just add it
# into the package itself.
#
# The .deb file is modified in place.

set -euo pipefail

DEB_PATH="${1:-}"

if [ -z "$DEB_PATH" ]; then
	echo "Usage: $0 <path-to-deb>" >&2
	exit 1
fi

if [ ! -f "$DEB_PATH" ]; then
	echo "error: file not found: $DEB_PATH" >&2
	exit 1
fi

DEB_PATH="$(readlink -f "$DEB_PATH")"

WORK_DIR="$(mktemp -d)"
readonly WORK_DIR
trap 'rm -rf "$WORK_DIR"' EXIT

echo "repackaging $DEB_PATH to add /usr/bin/but symlink..."

dpkg-deb -R "$DEB_PATH" "$WORK_DIR/pkg"

if [ ! -f "$WORK_DIR/pkg/usr/bin/gitbutler-tauri" ]; then
	echo "error: /usr/bin/gitbutler-tauri not found in package" >&2
	exit 1
fi

ln -sf gitbutler-tauri "$WORK_DIR/pkg/usr/bin/but"

dpkg-deb --root-owner-group -Zgzip -b "$WORK_DIR/pkg" "$DEB_PATH"

echo "done: $DEB_PATH now contains /usr/bin/but -> gitbutler-tauri"
