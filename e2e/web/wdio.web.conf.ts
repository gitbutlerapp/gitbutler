import { existsSync, mkdirSync } from 'node:fs';

const ci = !!process.env.CI_E2E;

export const config: WebdriverIO.Config = {
	runner: 'local',
	tsConfigPath: './tsconfig.json',
	specs: ['./test/specs/**/*.ts'],
	exclude: [],
	maxInstances: ci ? 10 : 1,
	outputDir: './test-results',
	capabilities: [
		{
			browserName: 'chrome',
			'goog:chromeOptions': {
				args: ci ? ['headless', 'no-sandbox'] : ['no-sandbox']
			}
		}
	],
	logLevel: 'info',
	bail: 0,
	waitforTimeout: 10000,
	connectionRetryTimeout: 120000,
	connectionRetryCount: 3,
	framework: 'mocha',
	reporters: [
		'spec',
		[
			'video',
			{
				saveAllVideos: false,
				videoSlowdownMultiplier: 3,
				videoRenderTimeout: 5,
				outputDir: './test-results/videos'
			}
		]
	],
	mochaOpts: {
		ui: 'bdd',
		// This is _very_ long because we are compiling the app inside the first-run test case.
		timeout: 600000,
		retries: ci ? 3 : 0
	},

	onPrepare: function () {
		// Ensure output directories exist
		const videoDir = './test-results/videos';

		if (!existsSync(videoDir)) {
			mkdirSync(videoDir, { recursive: true });
		}
	},

	afterTest: async function (_test, _thing, { error, passed }) {
		if (!passed && error) {
			try {
				// Capture browser logs on test failure
				const logs = (await browser.getLogs('browser')) as any[];

				let logContent = '=== BROWSER LOGS ===\n';

				logs.forEach((log) => {
					logContent += `[${log.timestamp}] ${log.level}: ${log.message}\n`;
				});

				// eslint-disable-next-line no-console
				console.log(logContent);
			} catch (logError) {
				console.error('Failed to capture browser logs:', logError);
			}
		}
	}
};
