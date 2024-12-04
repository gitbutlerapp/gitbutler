#!/usr/bin/env bash

set -eu -o pipefail

DATA_DIR="$HOME/.local/share/com.gitbutler.app.test"
if [ ! -d "$DATA_DIR" ]; then
  echo "Creating data dir: $DATA_DIR"
  mkdir -p $DATA_DIR
fi
echo "Confirming analytics"
echo '{"appAnalyticsConfirmed":true}' > $DATA_DIR/settings.json

