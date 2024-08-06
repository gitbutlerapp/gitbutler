import { browser } from '@wdio/globals';
import video from 'wdio-video-reporter';
import { spawn, ChildProcess } from 'node:child_process';
import { writeFile } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import type { Options, Frameworks } from '@wdio/types';

let tauriDriver: ChildProcess;

export const config: Options.WebdriverIO = {
	hostname: '127.0.0.1',
	port: 4444,
	specs: ['./e2e/**/*.spec.js'],
	maxInstances: 1,
	capabilities: [
		{
			// @ts-expect-error custom tauri capabilities
			'tauri:options': {
				application: '../../target/release/git-butler-dev'
			}
		}
	],
	reporters: [
		[
			video,
			{
				saveAllVideos: true,
				outputDir: './e2e/videos/'
			}
		],
		'spec'
	],
	framework: 'mocha',
	mochaOpts: {
		ui: 'bdd',
		timeout: 60000
	},
	autoCompileOpts: {
		autoCompile: true,
		tsNodeOpts: {
			project: './tsconfig.json',
			transpileOnly: true
		}
	},

	waitforTimeout: 10000,
	connectionRetryTimeout: 120000,
	connectionRetryCount: 3,

	// ensure we are running `tauri-driver` before the session starts so that we can proxy the webdriver requests
	beforeSession: () =>
		(tauriDriver = spawn(path.resolve(os.homedir(), '.cargo', 'bin', 'tauri-driver'), [], {
			stdio: [null, process.stdout, process.stderr]
		})),

	afterTest: async function (test: Frameworks.Test, result: Frameworks.TestResult) {
		if (result.error) {
			console.log('ERROR', result.error);
		}

		const screenshot = await browser.takeScreenshot();
		await writeFile(
			`./e2e/screenshots/${test.title.replaceAll(' ', '-').toLowerCase()}_${new Date().getTime()}.png`,
			screenshot,
			'base64'
		);
	},

	afterSession: () => tauriDriver.kill()
};
