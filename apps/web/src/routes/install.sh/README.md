# Install Script Endpoint

This directory serves the GitButler CLI installation script at `https://gitbutler.com/install.sh`.

## How it works

- **Source**: The script is imported from `scripts/install.sh` in the repository root
- **Alias**: Uses the `$scripts` alias (defined in `svelte.config.js`) to avoid brittle relative paths
- **Import method**: Uses Vite's `?raw` suffix to import the script as a plain string at build time
- **Deployment**: Works in Vercel's serverless environment since the script is bundled at build time

## Usage

Users can install GitButler CLI with:

```bash
curl -sSL https://gitbutler.com/install.sh | bash
```

## Testing

### Unit Tests

Located in `install.test.ts`, verifies:

- Script can be imported via the `$scripts` alias
- Script contains critical installation steps
- Script has proper bash structure and error handling

Run with:

```bash
cd apps/web
pnpm test
```

### E2E Tests

Located in `tests/install-script.spec.ts`, verifies:

- Script is served at the `/install.sh` endpoint
- Correct HTTP headers (content-type, caching)
- Script is downloadable with curl-like headers
- Response contains valid bash script

Run with:

```bash
cd apps/web
pnpm test:e2e:web
```

### CI

The GitHub workflow `.github/workflows/test-web.yml` runs automatically when:

- Changes are made to `apps/web/**`
- Changes are made to `scripts/install.sh`
- Pull requests affecting these paths

## Modifying the install script

When you modify `scripts/install.sh`:

1. The changes are automatically reflected at the `/install.sh` endpoint (via the `$scripts` alias)
2. CI tests will run to verify the endpoint still works
3. Both unit and e2e tests validate the script structure

No additional steps needed - the alias ensures the path always resolves correctly.
