#!/bin/sh
# Postinstall script for the rpm package.

set -eu

# Create a 'but' symlink so the CLI is available as /usr/bin/but.
#
# Ideally this symlink would be a proper tracked file in the RPM (like VS Code
# does since 2022: https://github.com/microsoft/vscode/pull/142907), but Tauri
# does not expose any way to add symlinks to packages even though the underlying
# rpm-rs crate supports them. Repackaging the RPM after the fact to inject a
# tracked symlink is possible but painful, so we use a postinstall script as a
# pragmatic workaround for now.
#
# We should consider contributing a patch upstream to Tauri to add support for
# adding this symlink as part of Tauri's bundling.
#
# We should also consider just building the RPM package from scratch. VS Code
# has a really good spec template that we could draw inspiration from. This
# would give us full control, and it really isn't hard.
SYMLINK=/usr/bin/but
TARGET=gitbutler-tauri

if [ ! -e "$SYMLINK" ] && [ ! -L "$SYMLINK" ]; then
    # Symlink does not exist; create it
    ln -s "$TARGET" "$SYMLINK"
elif [ -L "$SYMLINK" ]; then
    CURRENT_TARGET=$(readlink "$SYMLINK")
    case "$CURRENT_TARGET" in
        *gitbutler-tauri)
            # Points to gitbutler-tauri; update to current location
            ln -sf "$TARGET" "$SYMLINK"
            ;;
        *)
            echo "Error: $SYMLINK already exists and points to '$CURRENT_TARGET', not '$TARGET'" >&2
            exit 1
            ;;
    esac
else
    # Path exists but is not a symlink
    echo "Error: $SYMLINK already exists and is not a symlink" >&2
    exit 1
fi

# Print metrics notice on fresh install
if [ "${1:-}" = "1" ]; then
  echo ""
  echo "GitButler uses metrics to help us improve our product."
  echo "You can configure metrics collection either in the GUI or via 'but config metrics'."
  echo "Privacy policy: https://gitbutler.com/privacy"
  echo ""
fi
