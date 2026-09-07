#!/bin/bash

set -eu -o pipefail

# A distinct GitButler storage path configured for each app channel.
git init
git config gitbutler.storagePath gitbutler-release
git config gitbutler.nightly.storagePath gitbutler-nightly
git config gitbutler.dev.storagePath gitbutler-dev
