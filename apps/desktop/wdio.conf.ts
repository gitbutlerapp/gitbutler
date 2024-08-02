import { browser } from '@wdio/globals';
import { spawn, ChildProcess } from 'node:child_process';
import os from 'node:os';
import path from 'node:path';
import type { Options } from '@wdio/types';

let tauriDriver: ChildProcess;

export const config: Options.WebdriverIO = {
	hostname: '127.0.0.1',
	port: 4444,
	specs: ['./e2e/wdio/**/*.js'],
	maxInstances: 1,
	capabilities: [
		{
			// @ts-expect-error custom tauri capabilities
			'tauri:options': {
				application: '../../target/release/git-butler-dev'
			}
		}
	],
	reporters: ['spec'],
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

	afterTest: function ({ error }: { error: Error }) {
		if (error) {
			browser.takeScreenshot();
		}
	},

	afterSession: () => tauriDriver.kill()
};
