#!/bin/sh
# Clean up the 'but' symlink created by the postinstall script.
#
# RPM passes $1 as the number of package instances that will remain after this
# action completes:
#   0 = full uninstall
#   >0 = upgrade (new version already installed or about to be)
# We only remove the symlink on a full uninstall; during an upgrade the
# postinstall script of the new package will recreate/update it.

set -eu

# Skip removal during upgrades
if [ "${1:-0}" -ge 1 ]; then
    exit 0
fi

SYMLINK=/usr/bin/but

if [ -L "$SYMLINK" ]; then
    CURRENT_TARGET=$(readlink "$SYMLINK")
    case "$CURRENT_TARGET" in
        *gitbutler-tauri)
            rm -f "$SYMLINK"
            ;;
        *)
            echo "Warning: $SYMLINK points to '$CURRENT_TARGET', not gitbutler-tauri; leaving it alone" >&2
            ;;
    esac
elif [ -e "$SYMLINK" ]; then
    echo "Warning: $SYMLINK exists but is not a symlink; leaving it alone" >&2
fi
