## Frontend Tests

You can use the following commands to run front end tests.

- `pnpm test` - All unit tests (desktop, web, shared, ui packages)
- `pnpm test:ct` - Component tests (@gitbutler/ui with Playwright)
- `pnpm test:e2e:playwright` - E2E tests (Playwright)
- `pnpm test:e2e` - E2E tests (WebdriverIO, non-Tauri)
- `pnpm test:e2e:blackbox` - Blackbox E2E tests (WebdriverIO)

### Running Specific Test Files

**Component tests (Playwright)**: Pass the test file name as an argument (without `-t`):

```bash
# Run a specific component test file
pnpm test:ct -- HardWrapPlugin.spec

# Run tests matching a pattern
pnpm test:ct -- "HardWrap.*"
```

**Unit tests (Vitest)**: Navigate to the specific package and use the `-t` flag:

```bash
# Run tests in the ui package matching a pattern
cd packages/ui && pnpm test -- -t BranchLane

# Run tests in the desktop package
cd apps/desktop && pnpm test -- -t myComponent.test

# Run tests matching a pattern
cd packages/shared && pnpm test -- -t "pattern.*"
```

These commands allow you to run individual test files or groups of tests without running the entire test suite.
