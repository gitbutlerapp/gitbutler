#!/usr/bin/env bash
# Box bootstrap for mesh.but.dev (Amazon Linux 2023, arm64). Idempotent: runs as user-data at
# launch and can be re-run over SSM. Installs git and Caddy, the `mesh` user and its directories,
# and the systemd units. It doesn't install the hub itself; install.sh does, from a release.
set -euo pipefail

log() { echo "[box-setup] $*"; }

log "installing packages"
# The hub serves and receives pushes through `git http-backend`.
dnf install -y git tar gzip >/dev/null

CADDY_VERSION="${MESH_CADDY_VERSION:-2.11.3}"
if ! command -v caddy >/dev/null 2>&1; then
  log "installing caddy ${CADDY_VERSION}"
  curl -fsSL "https://github.com/caddyserver/caddy/releases/download/v${CADDY_VERSION}/caddy_${CADDY_VERSION}_linux_arm64.tar.gz" \
    | tar -xz -C /usr/bin caddy
  chmod +x /usr/bin/caddy
  /usr/bin/caddy version >/dev/null
fi
id caddy >/dev/null 2>&1 || useradd --system --home-dir /var/lib/caddy --create-home --shell /usr/sbin/nologin caddy
id mesh >/dev/null 2>&1 || useradd --system --home-dir /var/lib/mesh --create-home --shell /usr/sbin/nologin mesh
mkdir -p /var/lib/mesh/data /var/lib/mesh/app /opt/mesh
chown -R mesh:mesh /var/lib/mesh

here="$(dirname "$0")"
install -m 644 "$here/mesh.service" /etc/systemd/system/mesh.service
cat > /etc/systemd/system/caddy.service <<'UNIT'
[Unit]
Description=Caddy (TLS for mesh.but.dev)
After=network-online.target
Wants=network-online.target

[Service]
User=caddy
Group=caddy
Environment=XDG_DATA_HOME=/var/lib/caddy
Environment=XDG_CONFIG_HOME=/var/lib/caddy
ExecStart=/usr/bin/caddy run --config /etc/caddy/Caddyfile
ExecReload=/usr/bin/caddy reload --config /etc/caddy/Caddyfile --force
AmbientCapabilities=CAP_NET_BIND_SERVICE
Restart=always
RestartSec=3

[Install]
WantedBy=multi-user.target
UNIT
mkdir -p /etc/caddy
install -m 644 "$here/Caddyfile" /etc/caddy/Caddyfile

systemctl daemon-reload
systemctl enable caddy mesh
systemctl restart caddy   # serves TLS straight away; 502s until a release is installed
log "done"
