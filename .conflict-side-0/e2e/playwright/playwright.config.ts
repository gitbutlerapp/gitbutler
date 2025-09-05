import { BUT_SERVER_PORT, DESKTOP_PORT } from './src/env.ts';
import { defineConfig, devices } from '@playwright/test';
import path from 'node:path';

/**
 * Read environment variables from file.
 * https://github.com/motdotla/dotenv
 */
// import dotenv from 'dotenv';
// import path from 'path';
// dotenv.config({ path: path.resolve(__dirname, '.env') });

const AMOUNT_OF_WORKERS = 2;

/**
 * See https://playwright.dev/docs/test-configuration.
 */
export default defineConfig({
	testDir: './tests',
	/* Run tests in files in parallel */
	fullyParallel: true,
	/* Fail the build on CI if you accidentally left test.only in the source code. */
	forbidOnly: !!process.env.CI,
	/* Retry on CI only */
	retries: process.env.CI ? 2 : 0,
	/* Opt out of parallel tests on CI. */
	workers: process.env.CI ? AMOUNT_OF_WORKERS : undefined,
	/* Reporter to use. See https://playwright.dev/docs/test-reporters */
	reporter: process.env.CI ? 'github' : 'list',
	/* Shared settings for all the projects below. See https://playwright.dev/docs/api/class-testoptions. */
	use: {
		/* Base URL to use in actions like `await page.goto('/')`. */
		baseURL: `http://localhost:${DESKTOP_PORT}`,

		/* Collect trace when retrying the failed test. See https://playwright.dev/docs/trace-viewer */
		trace: 'on-first-retry',
		video: 'on-first-retry'
	},

	/* Configure projects for major browsers */
	projects: projects(),

	/* Run your local dev server before starting the tests */
	webServer: webServers()
});

function projects() {
	const projects = [];
	if (process.env.CI) {
		projects.push({
			name: 'chromium',
			use: { ...devices['Desktop Chrome'] }
		});
		return projects;
	}

	projects.push({
		name: 'Chrome',
		use: { ...devices['Desktop Chrome'], channel: 'chrome', headless: false }
	});

	return projects;
}

/**
 * Command to start the frontend server.
 * @param desktopPort - Optional port for the desktop application.
 */
function feServerCommand(desktopPort?: number) {
	const port = desktopPort ?? DESKTOP_PORT;
	return process.env.CI
		? `pnpm start:desktop --port ${port}`
		: `pnpm --filter @gitbutler/desktop dev --port ${port}`;
}

/**
 * Returns the Frontend server configuration for the playwright tests.
 *
 * If running locally, this returns a single server configuration what starts the dev server
 * on the default port.
 *
 * If running on CI, this returns multiple server configurations, one for each worker.
 * Each worker will start the dev server on a different port, so that they can run in parallel.
 */
function webServers() {
	const baseConfig = {
		cwd: path.resolve(import.meta.dirname, '../..'),
		command: feServerCommand(),
		url: `http://localhost:${DESKTOP_PORT}`,
		env: {
			VITE_BUTLER_PORT: BUT_SERVER_PORT,
			VITE_BUTLER_HOST: 'localhost',
			VITE_BUILD_TARGET: 'web'
		},
		reuseExistingServer: !process.env.CI,
		stdout: 'pipe'
	} as const;

	if (!process.env.CI) {
		return [baseConfig];
	}

	const feServerConfigs = [];
	for (let i = 0; i < AMOUNT_OF_WORKERS; i++) {
		const butServerPort = parseInt(BUT_SERVER_PORT, 10) + i;
		const desktopPort = parseInt(DESKTOP_PORT, 10) + i;

		feServerConfigs.push({
			...baseConfig,
			command: feServerCommand(desktopPort),
			url: `http://localhost:${desktopPort}`,
			env: {
				...baseConfig.env,
				VITE_BUTLER_PORT: `${butServerPort}`
			}
		} as const);
	}

	return feServerConfigs;
}
