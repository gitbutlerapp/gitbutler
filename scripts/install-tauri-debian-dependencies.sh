#!/bin/bash

set -eu -o pipefail

# Install the dependencies needed to build tauri, mainly.

set -x
apt update
apt install libwebkit2gtk-4.1-dev \
            build-essential \
            curl \
            wget \
            file \
            libxdo-dev \
            libssl-dev \
            libayatana-appindicator3-dev \
            librsvg2-dev \
            cmake
