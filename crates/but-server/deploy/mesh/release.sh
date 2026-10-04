#!/usr/bin/env bash
# Build and install a release on mesh.but.dev, from the repository root:
#   crates/but-server/deploy/mesh/release.sh
# Needs Docker (for the linux/arm64 build), Node 24 with pnpm, and AWS credentials for the account.
set -euo pipefail

REGION="${MESH_REGION:-us-east-1}"
INSTANCE="${MESH_INSTANCE:?set MESH_INSTANCE to the instance id}"
BUCKET="${MESH_BUCKET:?set MESH_BUCKET to the deploy bucket}"
here="$(cd "$(dirname "$0")" && pwd)"
root="$(git -C "$here" rev-parse --show-toplevel)"
sha="$(git -C "$root" rev-parse --short=12 HEAD)"
out="$(mktemp -d)"
trap 'rm -rf "$out"' EXIT

echo "[release] building but-server for linux/arm64"
docker build --platform linux/arm64 -t mesh-build -f "$here/build.Dockerfile" "$here"
# Cargo's registry and target live in volumes, apart from the host's own target directory.
docker run --rm --platform linux/arm64 \
  -v "$root:/src:ro" -v mesh-cargo:/root/.cargo/registry -v mesh-rustup:/root/.rustup -v mesh-target:/target \
  -e CARGO_TARGET_DIR=/target -w /src mesh-build \
  cargo build --release --locked -p but-server
docker run --rm --platform linux/arm64 -v mesh-target:/target -v "$out:/out" mesh-build \
  cp /target/release/but-server /out/but-server

echo "[release] building the web bundle"
(cd "$root" && pnpm -F @gitbutler/lite build:web)
cp -R "$root/apps/lite/dist/web" "$out/web"

# Without macOS's own metadata, which the box's GNU tar warns about for every file.
export COPYFILE_DISABLE=1
tar --no-xattrs -czf "$out/release.tgz" -C "$out" but-server web
tar --no-xattrs -czf "$out/deploy.tgz" -C "$here" box-setup.sh install.sh mesh.service Caddyfile
aws s3 cp --quiet "$out/release.tgz" "s3://$BUCKET/releases/$sha.tgz" --region "$REGION"
aws s3 cp --quiet "$out/deploy.tgz" "s3://$BUCKET/deploy.tgz" --region "$REGION"

echo "[release] installing $sha"
command_id="$(aws ssm send-command --region "$REGION" --instance-ids "$INSTANCE" \
  --document-name AWS-RunShellScript --comment "mesh release $sha" \
  --parameters "commands=[\"set -e\",\"mkdir -p /opt/mesh-deploy\",\"aws s3 cp --quiet s3://$BUCKET/deploy.tgz /tmp/deploy.tgz\",\"tar -xzf /tmp/deploy.tgz -C /opt/mesh-deploy\",\"bash /opt/mesh-deploy/box-setup.sh\",\"bash /opt/mesh-deploy/install.sh s3://$BUCKET/releases/$sha.tgz\"]" \
  --query Command.CommandId --output text)"
aws ssm wait command-executed --region "$REGION" --command-id "$command_id" --instance-id "$INSTANCE" || true
aws ssm get-command-invocation --region "$REGION" --command-id "$command_id" --instance-id "$INSTANCE" \
  --query '[Status,StandardOutputContent,StandardErrorContent]' --output text
