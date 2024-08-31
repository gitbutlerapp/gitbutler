import { TestRecorder } from './e2e/record.js';
import { spawn, type ChildProcess } from 'node:child_process';
import os from 'node:os';
import path from 'node:path';
import type { Options, Frameworks } from '@wdio/types';

const videoRecorder = new TestRecorder();
let tauriDriver: ChildProcess;

export const config: Options.WebdriverIO = {
	hostname: '127.0.0.1',
	port: 4444,
	specs: ['./e2e/tests/**/*.spec.ts'],
	maxInstances: 1,
	capabilities: [
		{
			// @ts-expect-error custom tauri capabilities
			maxInstances: 1,
			'tauri:options': {
				application: '../../target/debug/gitbutler-tauri'
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
	connectionRetryCount: 0,

	beforeTest: function (test: Frameworks.Test) {
		const videoPath = path.join(import.meta.dirname, '/e2e/videos');
		videoRecorder.start(test, videoPath);
	},

	afterTest: function () {
		videoRecorder.stop();
	},

	// ensure we are running `tauri-driver` before the session starts so that we can proxy the webdriver requests
	beforeSession: () =>
		(tauriDriver = spawn(path.resolve(os.homedir(), '.cargo', 'bin', 'tauri-driver'), [], {
			stdio: [null, process.stdout, process.stderr]
		})),

	afterSession: () => {
		tauriDriver.kill();
	}
};
