#!/usr/bin/env bash
# Install a release on the box, over SSM:
#   sudo /opt/mesh-deploy/install.sh s3://<bucket>/releases/<sha>.tgz
# Replaces the binary and web bundle, restarts the hub and checks it answers.
set -euo pipefail

RELEASE="${1:?usage: install.sh s3://bucket/releases/<sha>.tgz}"
[[ "$RELEASE" =~ ^s3://[a-z0-9.-]+/releases/[0-9a-f]{7,40}\.tgz$ ]] || { echo "[install] invalid release: $RELEASE" >&2; exit 2; }

staging="$(mktemp -d)"
trap 'rm -rf "$staging"' EXIT
aws s3 cp --quiet "$RELEASE" "$staging/release.tgz"
tar -xzf "$staging/release.tgz" -C "$staging"

install -m 755 "$staging/but-server" /opt/mesh/but-server
rm -rf /opt/mesh/web && cp -R "$staging/web" /opt/mesh/web
systemctl restart mesh

# Signed out, the hub answers its session check with 401.
for _ in $(seq 1 30); do
  if [ "$(curl -s -o /dev/null -w '%{http_code}' http://127.0.0.1:6980/session)" = 401 ]; then
    echo "[install] healthy: $RELEASE"
    exit 0
  fi
  sleep 1
done
echo "[install] not answering; recent logs:" >&2
journalctl -u mesh --no-pager -n 60 >&2
exit 1
