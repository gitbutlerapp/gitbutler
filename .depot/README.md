# Depot CI trial

`workflows/push.yaml` is a parallel, secret-free Linux subset of
`.github/workflows/push.yaml`, not a replacement. GitHub CI stays unchanged and
authoritative. No repository secrets need configuring in Depot.

## Setup and first run

1. Install Depot CLI and connect Depot Code Access app to this repository.
2. Run `depot ci migrate preflight` to verify authentication and repository access.
3. Test against local working tree:

   ```sh
   depot ci run --workflow .depot/workflows/push.yaml --job check-no-persist-credentials
   depot ci run --workflow .depot/workflows/push.yaml
   ```

Change-filtered jobs need matching changes. Changes under `.depot/**` select all
remaining test suites. Automatic push/PR triggers register after workflow merges
into default branch. Depot account/CLI authentication and app-provided repository
access are still required; these aren't manually configured workflow secrets.

## Scope and differences

- Linux jobs use explicit `depot-ubuntu-24.04` sandboxes (2 CPUs / 8 GB).
  Existing pinned actions, caches, and remaining test commands are retained.
- Node setup action is copied to `.depot/actions/init-env-node/action.yaml`.
  Keep this copy and GitHub action in sync during trial.
- GHCR-dependent jobs are omitted: `rust-lint`, `rust-test-but-api-macros`,
  `rust-test-tauri`, and `test-ui`. Their CI image requires registry credentials
  unavailable for this trial. These checks still run on GitHub.
- External test analytics credentials/configuration and Rust result upload step
  are removed. Existing JS reporters may log missing-token warnings but skip
  uploads without a token. Lite failure artifact upload remains; it uses CI's
  built-in artifact service, not a configured secret.
- Windows check, compatibility smoke workflow, and entire installer
  build/validate/publish chain remain exclusively on GitHub. Installer jobs require
  macOS and Ubuntu 22.04 coverage; Depot CI offers Ubuntu 24.04 sandboxes, not
  macOS/Windows. Depot-hosted GitHub Actions runners are a separate product.
- `check-rust` summarizes only remaining Linux jobs. Do not replace GitHub's
  required check with it. Fork PR triggers are currently unsupported by Depot CI;
  GitHub coverage must remain for external contributors.
- Checkout credential scan covers both workflow directories.

## Before any cutover

- Validate first-party PR and master runs, conditional skips, cancellation,
  cache restore/save, artifacts, and Lite Electron sandbox behavior on Depot.
- Compare remaining suites against GitHub. Restore omitted coverage before
  considering this workflow equivalent; never enable publishing in both systems.
- Keep existing required GitHub checks. This reduced trial cannot replace them.
- Application APIs, CLI/TUI, N-API, SDK, desktop/web/Lite source are unchanged:
  this migration changes CI orchestration only.

## References

- [Depot CI quickstart](https://depot.dev/docs/ci/quickstart)
- [GitHub Actions compatibility](https://depot.dev/docs/ci/compatibility)
- [Sandbox platforms and sizes](https://depot.dev/docs/ci/overview#depot-ci-sandboxes)
