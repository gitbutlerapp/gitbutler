## General information

This is a Rust/Svelte/React/TypeScript monorepo with multiple projects. For
agent instruction precedence and scoped guidance, follow the nearest applicable
`AGENTS.md`; use this file as Copilot-oriented project context.

The main applications are found in the `apps` directory:

- `desktop` containing the Tauri application's frontend code
- `web` containing the web application's frontend code
- `but-ui-pantry` containing the pantry, the design system's site

Rust code is found in the `crates` directory. It contains `gitbutler-tauri` for
the Tauri application, `but-api` for the API surface used by Tauri, CLI,
and TUI callers, and the `but` CLI crate.

The `packages` directory contains different self-contained npm packages.
These are shared between the applications.
The packages are:

- `but-sdk` containing the TypeScript types generated from the Rust API
- `but-ui` containing the design tokens, fonts and base styles
- `but-ui-react` containing the React component library built on `but-ui`
- `but-mcp-app` containing the React apps that render GitButler's MCP tool results
- `ui-svelte` containing the Svelte component library used by desktop and web
- `core` containing shared TypeScript helpers
- `shared` containing the services, state and utils shared between desktop and web
- `no-cross-domain-imports` containing the no-cross-domain-imports ESLint package
- `no-relative-imports` containing the no-relative-imports ESLint package
- `svelte-comment-injector` containing the Svelte comment injector package

The `e2e` directory contains Playwright, WebdriverIO, and blackbox end-to-end
tests.

## Development Setup

### Prerequisites

1. **Rust**: as specified in `rust-toolchain.toml`. Install via rustup.
2. **pnpm**: Install standalone pnpm as described in [DEVELOPMENT.md](../DEVELOPMENT.md#prerequisites).
3. **System Dependencies**: Tauri requires platform-specific dependencies (see DEVELOPMENT.md for details).

### Initial Setup

```bash
# Install dependencies and the project-managed Node.js runtime
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

# Run the pantry: the design notes and every component's page
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

When asked about frontend tests please read `@frontend.md` for context.

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
pnpm prettier

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
- Double quotes
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

**temporary directories** - use `but-testsupport` for scenario creation. Read-only functions should use read-only fixtures. DO NOT USE the `std::env::temp_dir().join(format!(…)` pattern.

## Dependency Management

- Workspace-level Rust dependencies go in `/Cargo.toml` under `[workspace.dependencies]`
- Individual crate dependencies reference workspace versions when possible, and when it's used more than once.

## Crate Organization

The `crates/` directory contains ~65 Rust crates organized by functionality:

- `gitbutler-*` prefix: Core GitButler functionality. These are old, and we want to port them to `but-*`.
- `but-*` prefix: Backend utilities and services
- Many crates have specific purposes (see DEVELOPMENT.md "Code Hitlist" for technical debt items)

**Note**: Some crates are marked for refactoring (see DEVELOPMENT.md). Be cautious when modifying:

- gitbutler-reference
- gitbutler-branch-actions
- gitbutler-repository

## CI/CD

### Workflows

Located in `.github/workflows/`:

- `push.yaml`: Main CI for linting, building, and testing on push
- `publish.yaml`: Release builds for different platforms
- `test-e2e.yml`: E2E tests (Playwright and blackbox)
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

- Use absolute imports via package references (e.g., `@gitbutler/ui-svelte`) instead of relative imports
- Shared utilities go in `packages/shared`
- UI components go in `packages/ui-svelte`

### Naming Conventions

**JavaScript/TypeScript**:

- Components: PascalCase (e.g., `BranchCard.svelte`)
- Files: kebab-case (e.g., `branch-service.ts`)
- Variables/functions: camelCase

### Error Handling

**Rust**: Use `Result<T, E>` and `anyhow` for error handling. Most functions should return `anyhow::Result`.

## Troubleshooting

### Build Issues

```bash
# Clear Turbo cache
pnpm exec turbo daemon stop
pnpm exec turbo daemon clean

```

### Platform-Specific Issues

- **macOS**: Ensure Xcode and cmake are installed
- **Linux**: Install webkit2gtk and other dependencies (see DEVELOPMENT.md)
- **Windows**: May require Perl, OpenSSL setup, and specific compiler settings (see DEVELOPMENT.md)

## Performance Considerations

### Development Builds

- Set `GITBUTLER_PERFORMANCE_LOG=1` for performance logging

### Turbo Cache

- Case-sensitive filesystem issues may affect Turbo on macOS (see DEVELOPMENT.md)

## Additional Resources

- **Development Guide**: See DEVELOPMENT.md for detailed setup instructions
- **Contributing Guide**: See CONTRIBUTING.md for contribution guidelines
- **Discord**: https://discord.gg/MmFkmaJ42D for questions and discussions
- See `crates/AGENTS.md` for Rust-specific API, repository, and testing conventions

## Important Notes for AI Coding Agents

1. **Check both JS and Rust** - changes to the backend often require frontend updates
2. **Code marked for refactoring**: Be extra careful with crates in the "Code Hitlist" section
3. **but CLI happy path testing only**: CLI tests are expensive and should be limited to what really matters.

## Review Guidelines

Follow these guidelines when reviewing code.

- Do not report type errors. CI will catch those.
- Don't review formatting such as commas, indentation, etc
