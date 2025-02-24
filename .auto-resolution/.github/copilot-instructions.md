## General information

This is a monorepo with multiple projects.
The main applications are found in the `apps` directory.
They are:

- `desktop` containing the Tauri application's frontend code
- `web` containing the web application's frontend code

The backend of the Tauri application is found in the `crates` directory.
It contains different rust packages, with `gitbutler-tauri` for the tauri application,
and `but-api` for implementing various command-line utilities like `but-testing`
and `but`.

The `packages` directory contains different self-contained npm packages.
These are shared between the `desktop` and `web` applications.
The packages are:

- `ui` containing the shared UI components
- `shared` containing the shared types and utils
- `no-relative-imports` containing the no-relative-imports ESLINT package

## Technology Stack

- **Frontend**: Svelte with TypeScript
- **Backend**: Rust
- **Desktop Framework**: Tauri (like Electron but using Rust instead of Node.js)
- **Build Tools**: Turborepo for monorepo management, pnpm for package management
- **Git Libraries**: gix (gitoxide) and git2 (libgit2)
- **Testing**: Vitest for JS/TS, Playwright for E2E tests, standard Rust testing framework

## Development Setup

### Prerequisites

1. **Rust**: Version 1.91 (as specified in `rust-toolchain.toml`). Install via rustup.
2. **Node.js**: Version 20.11 or higher (specified in `package.json`). Use the version in `.nvmrc` (lts/jod, which resolves to Node 22).
3. **pnpm**: Version 10.17.0 (specified in `package.json`). Enabled via corepack. Run `corepack enable` in the project root.
4. **System Dependencies**: Tauri requires platform-specific dependencies (see DEVELOPMENT.md for details).

### Initial Setup

```bash
# Clone the repository
git clone https://github.com/gitbutlerapp/gitbutler.git
cd gitbutler

# Enable pnpm via corepack
corepack enable

# Install dependencies
pnpm install

# Build Rust binaries (required before running the app)
# look at https://github.com/gitbutlerapp/gitbutler/blob/fd9a58de5579074c0526193143d531c21907a26b/scripts/install-tauri-debian-dependencies.sh
# for Linux prerequisites for tauri.
cargo build

# or use this to build the `but` CLI, without the need for tauri dependencies.
cargo build -p but
```

## Building and Running

### Development

```bash
# Run desktop app in development mode
pnpm dev:desktop

# Run with debug logs
LOG_LEVEL=debug pnpm dev:desktop

# Run web app
pnpm dev:web

# Run UI component storybook
pnpm dev:ui
```

### Building

```bash
# Build all packages
pnpm build

# Build desktop app only
pnpm build:desktop

# Build for production (used for releases)
pnpm tauri build --features devtools --config crates/gitbutler-tauri/tauri.conf.nightly.json
```

## Testing

```bash
# Run all tests
pnpm test

# Run tests in watch mode
pnpm test:watch

# Run E2E tests with Playwright
pnpm test:e2e:playwright

# Run Rust tests
cargo test

# Run specific crate tests
cargo test -p gitbutler-branch-actions
```

## Code Style and Linting

### JavaScript/TypeScript

**Linting**: ESLint with TypeScript and Svelte plugins
**Formatting**: Prettier with Svelte plugin

```bash
# Check linting
pnpm lint

# Fix linting issues
pnpm fix

# Format code
pnpm format

# Check formatting
pnpm prettier --check .

# Shortcut to check everything
pnpm isgood

# Shortcut to fix and format everything
pnpm begood
```

**Important ESLint Rules**:

- No relative imports (use `@gitbutler/` package references)
- Import order: alphabetically sorted with specific group ordering
- No console.log (use console.warn or console.error)
- Prefer function declarations over arrow functions at top level
- Svelte-specific rules for button types, unused props, etc.

**Prettier Config**:

- Tabs for indentation
- Single quotes
- No trailing commas
- 100 character line width

### Rust

**Formatting**: Use rustfmt with nightly toolchain for imports grouping

```bash
# Format Rust code
cargo fmt

# Format with nightly settings
pnpm rustfmt
# or (but don't do this unless you are asked to)
cargo +nightly fmt -- --config-path rustfmt-nightly.toml

# Run clippy for linting
cargo clippy --all-targets
```

**Rust Formatting**:

- Group imports: Std, External, Crate
- Imports granularity: Crate level

## Dependency Management

### Adding JavaScript/TypeScript Dependencies

```bash
# Add to root workspace
pnpm add -D <package>

# Add to specific package
pnpm add <package> --filter @gitbutler/desktop
pnpm add <package> --filter @gitbutler/ui
```

**Always check for vulnerabilities before adding npm packages** (ecosystem: npm).

### Adding Rust Dependencies

Edit the appropriate `Cargo.toml` file:

- Workspace-level dependencies go in `/Cargo.toml` under `[workspace.dependencies]`
- Individual crate dependencies reference workspace versions when possible, and when it's used more than once.

**Always check for vulnerabilities before adding Rust dependencies** (ecosystem: rust).

## Monorepo Structure

### Workspace Navigation

The repository uses:

- **Turborepo** for JavaScript/TypeScript build orchestration
- **Cargo workspace** for Rust crate management
- **pnpm workspace** for npm package management

When making changes:

1. Identify which package/crate is affected
2. Run builds/tests in that package first
3. Then verify dependent packages still work

### Crate Organization

The `crates/` directory contains ~65 Rust crates organized by functionality:

- `gitbutler-*` prefix: Core GitButler functionality. These are old, and we want to port them to `but-*`.
- `but-*` prefix: Backend utilities and services
- Many crates have specific purposes (see DEVELOPMENT.md "Code Hitlist" for technical debt items)

**Note**: Some crates are marked for refactoring (see DEVELOPMENT.md). Be cautious when modifying:

- gitbutler-reference
- gitbutler-branch-actions
- gitbutler-repository
- gitbutler-url

## CI/CD

### Workflows

Located in `.github/workflows/`:

- `push.yaml`: Main CI for linting, building, and testing on push
- `publish.yaml`: Release builds for different platforms
- `test-e2e-playwright.yml`: E2E tests with Playwright
- `test-e2e-blackbox.yml`: E2E blackbox tests
- `test-client-fe-integration.yml`: Frontend integration tests

### Pre-commit Checks

Before committing, ensure:

1. Code is formatted: `pnpm format && pnpm rustfmt`
2. Linting passes: `pnpm lint && cargo clippy --all-targets`
3. Tests pass: `pnpm test && cargo test`
4. Build succeeds: `pnpm build`

Or use the shortcut: `pnpm isgood`.
Auto-fix with `pnpm begood && cargo clippy --fix --all-targets`

## Common Patterns and Conventions

### Frontend File Organization

- Use absolute imports via package references (e.g., `@gitbutler/ui`) instead of relative imports
- Components should be in logical directories by feature
- Shared utilities go in `packages/shared`
- UI components go in `packages/ui`

### Naming Conventions

**JavaScript/TypeScript**:

- Components: PascalCase (e.g., `BranchCard.svelte`)
- Files: kebab-case (e.g., `branch-service.ts`)
- Variables/functions: camelCase

**Rust**:

- Follow standard Rust naming conventions

### Error Handling

**JavaScript/TypeScript**: Throw errors and handle with try/catch at appropriate boundaries

**Rust**: Use `Result<T, E>` and `anyhow` for error handling. Most functions should return `anyhow::Result`.

## Troubleshooting

### Build Issues

```bash
# Clear Turbo cache
pnpm exec turbo daemon stop
pnpm exec turbo daemon clean

# Clear node_modules and reinstall
rm -rf .turbo node_modules
pnpm install

# Clear Rust build artifacts
cargo clean
```

### Node/pnpm Issues

```bash
# Use correct Node version
nvm install
nvm use

# Ensure pnpm is via corepack
corepack enable
corepack prepare pnpm@10.17.0 --activate
```

### Platform-Specific Issues

- **macOS**: Ensure Xcode and cmake are installed
- **Linux**: Install webkit2gtk and other dependencies (see DEVELOPMENT.md)
- **Windows**: May require Perl, OpenSSL setup, and specific compiler settings (see DEVELOPMENT.md)

## Performance Considerations

### Development Builds

- Set `GITBUTLER_PERFORMANCE_LOG=1` for performance logging
- Use `--release` flag for more realistic performance testing

### Turbo Cache

- Turborepo caches build outputs
- If experiencing stale builds, clear cache: `pnpm exec turbo daemon clean`
- Case-sensitive filesystem issues may affect Turbo on macOS (see DEVELOPMENT.md)

## Additional Resources

- **Development Guide**: See DEVELOPMENT.md for detailed setup instructions
- **Contributing Guide**: See CONTRIBUTING.md for contribution guidelines
- **Discord**: https://discord.gg/MmFkmaJ42D for questions and discussions
- See `crates/but/agents.md` for information about the `but` CLI specifically

## Important Notes for AI Coding Agents

1. **Always run lints and tests** after making changes
2. **Use the existing patterns** in the codebase - don't introduce new patterns without discussion
3. **Keep changes minimal** - surgical changes are preferred over large refactors
4. **Follow the monorepo structure** - understand which package your change affects
5. **Check both JS and Rust** - changes to the backend often require frontend updates
6. **Documentation** - update relevant docs if changing public APIs or user-facing features
7. **Avoid relative imports** - ESLint will error on relative imports; use package references
8. **Test on target platform** if making platform-specific changes
9. **Security**: Check dependencies for vulnerabilities before adding them
10. **Code marked for refactoring**: Be extra careful with crates in the "Code Hitlist" section
11. **but CLI happy path testing only**: CLI tests are expensive and should be limited to what really matters.
