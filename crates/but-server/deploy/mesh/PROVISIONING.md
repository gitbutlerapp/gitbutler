# Provisioning record: mesh.but.dev

Created 2026-10-04 in account 221886364861 (`us-east-1`), simplified from tests.but.dev's single-box pattern. It's the hosted Lite hub: `but-server --hosted-dir`, behind Caddy.

| Resource | Value |
|---|---|
| Deploy bucket | `mesh-deploy-221886364861`, public access blocked: `deploy.tgz` (these files) and `releases/<sha>.tgz` (binary and web bundle) |
| IAM role and instance profile | `mesh-instance`: `AmazonSSMManagedInstanceCore`, plus inline `mesh-inline` (read the deploy bucket) |
| Security group | `mesh` (`sg-07e9dd33812a0ee60`, default VPC `vpc-046099d13779da768`): 80 and 443 from anywhere, no SSH; access is through SSM only |
| Instance | `i-0f0398501914519fd`, `t4g.small`, Amazon Linux 2023 arm64 (`ami-065b1b834d2a83a7a`), Name=mesh, IMDSv2 required, 30 GB gp3 root volume, deleted on termination |
| Elastic IP | `184.194.197.230` (`eipalloc-0b1cbac4b21f3831f`) |
| DNS | `mesh.but.dev` A → 184.194.197.230 in zone `but.dev` (`Z07447682BGN2VZZABWM3`); overrides the `*.but.dev` wildcard for this name only |

## Simplified from tests.but.dev

- **No Docker or ECR.** `but-server` runs as a plain binary under systemd (`mesh.service`), as the `mesh` user. It needs `git` on the box for `git http-backend`.
- **No separate data volume or backups.** The hub's stores live in `/var/lib/mesh/data` on the root volume. Machines can publish everything again, so losing the box loses nothing that can't be recreated.
- **No CloudWatch.** Logs are in `journalctl -u mesh` and `journalctl -u caddy`.
- **Built locally.** `release.sh` builds the binary in an Amazon Linux 2023 container (`build.Dockerfile`), so it links against the box's glibc. It's native on Apple silicon, so a full build takes about 3 minutes.

## Layout on the box

- `/opt/mesh/but-server` and `/opt/mesh/web`: the installed release.
- `/var/lib/mesh/data`: the hub's per-user stores. `/var/lib/mesh/app`: its own settings (`E2E_TEST_APP_DATA_DIR`).
- `/opt/mesh-deploy`: these files, as last installed.

## Bootstrap

User data downloads `s3://mesh-deploy-221886364861/deploy.tgz` into `/opt/mesh-deploy` and runs `box-setup.sh`. Every release re-runs it, so changing these files and releasing applies them.

## Releases

From the repository root, with Docker running, Node 24 and AWS credentials for the account:

```sh
MESH_INSTANCE=i-0f0398501914519fd MESH_BUCKET=mesh-deploy-221886364861 crates/but-server/deploy/mesh/release.sh
```

It builds, uploads `releases/<sha>.tgz`, then runs `box-setup.sh` and `install.sh` over SSM. `install.sh` restarts the hub and checks it answers. To roll back, run `install.sh` over SSM with an earlier release's S3 path.

## Using it

Lite and the CLI use this hub by default. To use another, for example a local one, set `BUT_HOSTED_URL` before starting Lite or running the CLI:

```sh
BUT_HOSTED_URL=http://localhost:6980
```

Sign-in on the web page is the pasted-token shortcut, and any GitButler account can sign in and push repositories. Each account only sees its own stores, but nothing limits disk use.

## Tearing it down

Delete these in order: the DNS record, the Elastic IP (disassociate, then release), the instance, the security group, the instance profile and role, and the bucket.
