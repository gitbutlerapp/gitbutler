# End-to-End Testing

This directory contains all end-to-end (E2E) tests for GitButler. We use two different testing frameworks to cover different scenarios:

- **Playwright**: For testing the desktop application UI
- **WebdriverIO (WDIO)**: For blackbox testing and web application testing

## Table of Contents

- [Installation](#installation)
- [Running Tests](#running-tests)
- [Adding New Tests](#adding-new-tests)
- [Debugging Tests](#debugging-tests)
- [Test Structure](#test-structure)

## Installation

### Prerequisites

Before running E2E tests, ensure you have:

1. **Node.js** (v20.11+): See [DEVELOPMENT.md](../DEVELOPMENT.md) for installation
2. **pnpm** (v10.17.0): Enabled via `corepack enable` in the project root
3. **Built application**: E2E tests require a built version of the app

### Install Dependencies

From the project root:

```bash
# Install all dependencies
pnpm install

# Install Playwright browsers (required for Playwright tests)
cd e2e
pnpm install-playwright
```

### Build the Application

The E2E tests need a compiled version of the application:

```bash
# From project root - build the Rust backend
cargo build

# Build the frontend
pnpm build
```

## Running Tests

### Playwright Tests (Desktop UI)

Playwright tests run against the desktop application and test the UI interactions.

```bash
# From project root or e2e directory
pnpm test:e2e:playwright

# Run with Playwright UI mode (interactive debugging)
pnpm test:e2e:playwright:open
```

Playwright tests are located in `playwright/tests/` and test the desktop Tauri application.

### Blackbox Tests

Blackbox tests use WebdriverIO to test the application in a more isolated manner:

```bash
# From project root
pnpm test:e2e:blackbox

# Or from e2e directory
pnpm run test:e2e:blackbox
```

These tests require the Tauri driver and test the application as a complete black box.

### Web Tests (Not Tauri)

Tests for the web version of the application:

```bash
# From e2e directory
pnpm test:e2e:not-tauri
```

## Adding New Tests

### Adding a Playwright Test

1. **Create a new test file** in `playwright/tests/`:

```typescript
import { startGitButler, type GitButler } from "../src/setup.ts";
import { clickByTestId, waitForTestId } from "../src/util.ts";
import { expect, test } from "@playwright/test";

let gitbutler: GitButler;

test.afterEach(async () => {
	await gitbutler?.destroy();
});

test("should do something amazing", async ({ page, context }, testInfo) => {
	const workdir = testInfo.outputPath("workdir");
	const configdir = testInfo.outputPath("config");
	gitbutler = await startGitButler(workdir, configdir, context);

	// Setup test data
	await gitbutler.runScript("your-setup-script.sh");

	await page.goto("/");

	// Your test logic here
	await waitForTestId(page, "your-element");
	await clickByTestId(page, "your-button");

	// Assertions
	expect(await page.locator('[data-testid="result"]').textContent()).toBe("expected");
});
```

2. **Add setup scripts** if needed in `playwright/scripts/` for test data preparation

3. **Use test utilities** from `playwright/src/`:
   - `setup.ts`: For starting GitButler instances
   - `util.ts`: Helper functions for common UI interactions
   - `env.ts`: Environment configuration

### Adding a Blackbox Test

1. **Create a new test file** in `blackbox/tests/`:

```typescript
import { spawnAndLog, findAndClick, setElementValue } from "../utils.js";

describe("Your Feature", () => {
	before(() => {
		// Setup test repositories
		spawnAndLog("bash", [
			"-c",
			"./blackbox/scripts/your-setup-script.sh ../target/debug/gitbutler-cli",
		]);
	});

	it("should perform an action", async () => {
		await findAndClick('button[data-testid="your-button"]');

		const element = $('input[data-testid="your-input"]');
		await setElementValue(await element.getElement(), "test value");

		// Add assertions
		const result = await $('div[data-testid="result"]').getElement();
		await expect(result).toExist();
	});
});
```

2. **Add bash setup scripts** in `blackbox/scripts/` if you need to prepare test repositories

### Test Data and Fixtures

- **Playwright**: Test scripts are in `playwright/scripts/` - these are bash scripts that set up git repositories
- **Blackbox**: Setup scripts in `blackbox/scripts/` prepare test repositories
- **Fixtures**: Shared fixtures can be placed in `playwright/fixtures/`

### Best Practices

1. **Use data-testid attributes**: Always add `data-testid` attributes to elements you need to interact with in tests
2. **Clean up**: Always clean up test resources in `afterEach` or `after` hooks
3. **Isolation**: Each test should be independent and not rely on state from other tests
4. **Meaningful names**: Use descriptive test names that explain what is being tested
5. **Wait for elements**: Always wait for elements to be ready before interacting with them

## Debugging Tests

### Debugging Playwright Tests

#### 1. Interactive UI Mode

The easiest way to debug Playwright tests:

```bash
pnpm test:e2e:playwright:open
```

This opens Playwright's UI where you can:

- Run tests step by step
- See the browser in real-time
- Inspect the DOM at any point
- Time-travel through test execution

#### 2. Headed Mode

Run tests with the browser visible:

```bash
# Set PLAYWRIGHT_UI environment variable
PLAYWRIGHT_UI=1 pnpm test:e2e:playwright
```

#### 3. Debug Specific Test

Run a single test file:

```bash
cd e2e
pnpm exec playwright test --config ./playwright/playwright.config.ts tests/yourTest.spec.ts
```

Run a specific test by name:

```bash
pnpm exec playwright test --config ./playwright/playwright.config.ts -g "test name pattern"
```

#### 4. VSCode Debugging

Add breakpoints in your test files and use VSCode's built-in debugger:

1. Set breakpoints in your test code
2. Open the test file
3. Click "Debug Test" in the test explorer or use the Playwright extension

#### 5. Screenshots and Videos

Tests automatically capture:

- **Traces**: Enabled by default (see `playwright.config.ts`)
- **Videos**: Captured on failure (retained on failure)
- **Screenshots**: Can be added manually in tests

Access these in `e2e/test-results/` after test runs.

#### 6. Console Logs

Add logging to your tests:

```typescript
console.log("Debug info:", await page.locator('[data-testid="element"]').textContent());
```

View browser console in headed mode or check test output.

### Debugging Blackbox Tests

#### 1. Video Recording

Blackbox tests record videos of test runs. Videos are saved in `blackbox/videos/`.

#### 2. Increase Timeout

Edit timeouts in `blackbox/wdio.blackbox.conf.ts`:

```typescript
mochaOpts: {
	timeout: 120000; // Increase for debugging
}
```

#### 3. Run Specific Test

```bash
cd e2e
pnpm exec wdio run ./blackbox/wdio.blackbox.conf.ts --spec ./blackbox/tests/your-test.spec.ts
```

#### 4. Add Debug Statements

Use `browser.debug()` in your test to pause execution:

```typescript
it("should do something", async () => {
	await findAndClick('button[data-testid="test"]');
	await browser.debug(); // Pauses here
	// ... rest of test
});
```

### Common Debugging Issues

**Tests timing out:**

- Increase timeout in config files
- Check if elements are actually rendered (use browser DevTools)
- Verify `data-testid` attributes exist

**Cannot find elements:**

- Check that the element has a `data-testid` attribute
- Use `page.locator()` with a more general selector to verify element exists
- Check if element is in an iframe or shadow DOM

**Flaky tests:**

- Add proper waits (`waitForTestId`, `waitForSelector`)
- Avoid hardcoded sleeps - use proper wait conditions
- Check for race conditions in the application

**Application not starting:**

- Verify application is built: `cargo build`
- Check logs in terminal output
- Ensure no other instances are running on the same port

### Accessing Test Artifacts

After a test run:

```bash
# Playwright artifacts
ls e2e/test-results/

# View Playwright trace
pnpm exec playwright show-trace e2e/test-results/path-to-trace.zip

# Blackbox videos
ls e2e/blackbox/videos/
```

You can also download them from the respective E2E Job on CI, or find a link in the "Upload Artifact" job log.

## Test Structure

```
e2e/
├── playwright/              # Playwright tests for desktop app
│   ├── tests/              # Test files (*.spec.ts)
│   ├── src/                # Test utilities and helpers
│   │   ├── setup.ts        # GitButler instance setup
│   │   ├── util.ts         # UI interaction helpers
│   │   └── env.ts          # Environment config
│   ├── scripts/            # Bash scripts for test setup
│   ├── fixtures/           # Test fixtures and data
│   └── playwright.config.ts # Playwright configuration
│
├── blackbox/               # Blackbox tests using WebdriverIO
│   ├── tests/              # Test files (*.spec.ts)
│   ├── scripts/            # Repository setup scripts
│   ├── utils.ts            # Test utilities
│   ├── videos/             # Recorded test videos
│   └── wdio.blackbox.conf.ts # WDIO configuration
│
├── test-results/           # Test run artifacts
└── package.json            # E2E package configuration
```

## Configuration Files

- `playwright/playwright.config.ts`: Playwright test configuration
- `blackbox/wdio.blackbox.conf.ts`: WebdriverIO blackbox test configuration
- `package.json`: Test scripts and dependencies

## CI/CD

E2E tests run in CI via GitHub Actions:

- `.github/workflows/test-e2e-playwright.yml`: Playwright tests
- `.github/workflows/test-e2e-blackbox.yml`: Blackbox tests

Tests run on push and pull requests with:

- Automatic retries on CI (2 retries)
- Video capture on failures
- Trace collection for debugging

## Further Reading

- [Playwright Documentation](https://playwright.dev/)
- [WebdriverIO Documentation](https://webdriver.io/)
- [Project DEVELOPMENT.md](../DEVELOPMENT.md)
- [Contributing Guidelines](../CONTRIBUTING.md)
