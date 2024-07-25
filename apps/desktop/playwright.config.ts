import { defineConfig, devices } from '@playwright/test';

/**
 * See https://playwright.dev/docs/test-configuration.
 */
export default defineConfig({
	testDir: './e2e/playwright',
	testMatch: /(.+\.)?(test|spec)\.[jt]s/,
	projects: [
		{
			name: 'Google Chrome',
			use: { ...devices['Desktop Chrome'] }
		}
	],
	expect: {
		timeout: 20 * 1000
	},
	use: {
		trace: 'retain-on-failure'
	},
	webServer: {
		command: 'pnpm test:e2e:run',
		url: 'http://localhost:1420'
	}
});
