#!/usr/bin/env bash

MANIFEST_PATH="crates/gitbutler-tauri/flatpak/manifest.flatpak.yml"
FLATPAK_ID="com.gitbutler.app"

# Add Flathub Repository
flatpak remote-add --user --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo

# Install Gnome runtime dependency
flatpak install --user flathub org.gnome.Platform//47 org.gnome.Sdk//47

# Build the manifest
flatpak-builder --force-clean --user --install-deps-from=flathub --repo=repo builddir "$MANIFEST_PATH"

# Export gitbutler-tauri.flatpak
flatpak build-bundle repo "$FLATPAK_ID.flatpak" "$FLATPAK_ID"

# Notes
# - https://tauri.app/distribute/flatpak/
# - https://docs.flatpak.org/en/latest/first-build.html
# - https://github.com/madeofpendletonwool/PinePods/blob/5377b061357d1ba5e04e607b5f626bf349cf09f8/.github/workflows/build-flatpak.yml#L4
# - https://github.com/Beanow/hi-flatpak
# - https://github.com/flathub/com.vscodium.codium-insiders/blob/master/com.vscodium.codium-insiders.yaml
