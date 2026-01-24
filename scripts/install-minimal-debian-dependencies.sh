#!/bin/bash

set -eu -o pipefail

# Install the dependencies needed to build the but-server and the CLI.
apt update
apt install -y libdbus-1-dev libglib2.0-dev pkg-config
