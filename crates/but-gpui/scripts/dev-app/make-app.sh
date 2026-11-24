#!/usr/bin/env bash
# Build the client for release and write `GitButler GPUI Dev.app`, an app that runs that build
# from this checkout's `target` folder, for `make but-gpui-app-dev`. Open it from Finder,
# Spotlight or the Dock. Build and restart in it builds for release again and restarts the app
# on the new build, also one built in another worktree. The app opens the build that ran in it
# last.
#
#   APP_DIR   the folder to write the app in, ~/Applications by default.
#
# Build and restart runs cargo in the environment saved in the app: this checkout's direnv
# environment, as the nix devshell, when it has one, on top of the PATH this runs with. Run it
# again when that changes, as when the flake's toolchain does.
set -euo pipefail

if [[ $(uname) != Darwin ]]; then
    echo "make-app.sh: the dev app is for macOS" >&2
    exit 1
fi

here=$(cd "$(dirname "$0")" && pwd -P)
checkout=$(cd "$here/../../../.." && pwd -P)
app="${APP_DIR:-$HOME/Applications}/GitButler GPUI Dev.app"
cargo=$(command -v cargo)

# As Build and restart builds it, so its first build starts from this one.
"$cargo" build --manifest-path "$checkout/crates/but-gpui/Cargo.toml" --release \
    -p but-gpui -p gitbutler-git --bin gitbutler-gpui --bin gitbutler-git-askpass \
    --features gpui_platform/runtime_shaders
metadata=$("$cargo" metadata --manifest-path "$checkout/Cargo.toml" --format-version 1 --no-deps)
[[ $metadata =~ \"target_directory\":\"([^\"]+)\" ]]
target=${BASH_REMATCH[1]}

mkdir -p "$app/Contents/MacOS" "$app/Contents/Resources"
rustc --edition 2024 -O -o "$app/Contents/MacOS/launcher" "$here/launcher.rs"
ln -sfn "$target/release/gitbutler-gpui" "$app/Contents/MacOS/gitbutler-gpui"
cp "$here/icon.icns" "$app/Contents/Resources/icon.icns"

# The environment the launcher gives the client, NUL-separated as `env -0` prints it. As
# `cargo run` from this shell would give it, with this shell's SHELL for the terminals to start:
# the Dock's is the account's login shell, and the devshell's is its own bash. Also direnv's
# own variables, which say the devshell is loaded, so a terminal opened in this checkout doesn't
# load it again.
base=(env -i HOME="$HOME" USER="$USER" PATH="$PATH")
if command -v direnv >/dev/null && [[ -f $checkout/.envrc ]]; then
    "${base[@]}" direnv exec "$checkout" env -0
else
    "${base[@]}" env -0
fi | {
    while IFS= read -r -d '' variable; do
        name=${variable%%=*}
        case $name in
        # What macOS gives every app, what direnv sets only while it reads `.envrc`, and the
        # devshell's temporary folder, which nix deletes.
        HOME | USER | LOGNAME | PWD | OLDPWD | SHLVL | TMPDIR | TEMP | TMP | TEMPDIR | \
            NIX_BUILD_TOP | DIRENV_IN_ENVRC | SHELL | CARGO) ;;
        # The devshell also exports its derivation's attributes, as `shellHook`, in lowercase.
        *[![:upper:][:digit:]_]* | [![:upper:]]*) ;;
        *) printf '%s\0' "$variable" ;;
        esac
    done
    printf 'CARGO=%s\0' "$cargo"
    printf 'SHELL=%s\0' "${SHELL:-/bin/zsh}"
} >"$app/Contents/Resources/environment"

cat >"$app/Contents/Info.plist" <<'EOF'
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleExecutable</key>
    <string>launcher</string>
    <key>CFBundleIconFile</key>
    <string>icon</string>
    <key>CFBundleIdentifier</key>
    <string>com.gitbutler.gpui.dev</string>
    <key>CFBundleName</key>
    <string>GitButler GPUI Dev</string>
    <key>CFBundleDisplayName</key>
    <string>GitButler GPUI Dev</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleShortVersionString</key>
    <string>0.0.0</string>
    <key>CFBundleVersion</key>
    <string>0</string>
    <key>NSHighResolutionCapable</key>
    <true/>
</dict>
</plist>
EOF

# Launch Services keeps what it read of the app until told again, as an earlier one's
# `LSEnvironment`.
/System/Library/Frameworks/CoreServices.framework/Frameworks/LaunchServices.framework/Support/lsregister -f "$app"
echo "Wrote $app, which runs $target/release/gitbutler-gpui"
