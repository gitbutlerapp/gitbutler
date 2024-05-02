import { defineConfig, devices } from '@playwright/test';

/**
 * See https://playwright.dev/docs/test-configuration.
 */
export default defineConfig({
	// testDir: './e2e/playwright',
	// testMatch: './e2e/playwright/*.spec.ts',
	fullyParallel: true,
	forbidOnly: !!process.env.CI,
	retries: process.env.CI ? 3 : 0,
	workers: process.env.CI ? 1 : 1,
	reporter: 'html',
	// globalSetup: './e2e/playwright/globalSetup.ts',
	use: {
		launchOptions: {
			executablePath: '/home/ndo/.nix-profile/bin/chromium'
		},
		/* Base URL to use in actions like `await page.goto('/')`. */
		// baseURL: 'http://localhost:3000',

		/* Collect trace when retrying the failed test. See https://playwright.dev/docs/trace-viewer */
		trace: 'on-first-retry'
	},

	projects: [
		// Only Chromium supports connecting to SELENIUM_REMOTE
		{
			name: 'Google Chrome',
			use: { ...devices['Desktop Chrome'], channel: 'chrome' } // or 'chrome-beta'
		}
	]

	// webServer: {
	// 	command: 'pnpm exec vite preview --port 1420',
	// 	url: 'http://localhost:1420'
	// 	// reuseExistingServer: !process.env.CI
	// }
});
