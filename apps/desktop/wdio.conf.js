import { spawn } from 'node:child_process';
import os from 'node:os';
import path from 'node:path';
// import { browser } from "@wdio/globals";

let tauriDriver;

export const config = {
	hostname: '127.0.0.1',
	port: 4444,
	specs: ['./e2e/wdio/**/*.js'],
	maxInstances: 1,
	capabilities: [
		{
			'tauri:options': {
				application: '/opt/gitbutler/gitbutler/target/release/git-butler-dev'
			}
		}
	],
	reporters: ['spec'],
	framework: 'mocha',
	mochaOpts: {
		ui: 'bdd',
		timeout: 60000
	},

	// Level of logging verbosity: trace | debug | info | warn | error | silent
	logLevel: 'trace',

	waitforTimeout: 10000,
	connectionRetryTimeout: 120000,
	connectionRetryCount: 3,

	// ensure we are running `tauri-driver` before the session starts so that we can proxy the webdriver requests
	beforeSession: () =>
		(tauriDriver = spawn(path.resolve(os.homedir(), '.cargo', 'bin', 'tauri-driver'), [], {
			stdio: [null, process.stdout, process.stderr]
		})),

	afterTest: function (test, context, { error, result, duration, passed, retries }) {
		if (error) {
			browser.takeScreenshot();
		}
	},

	// clean up the `tauri-driver` process we spawned at the start of the session
	afterSession: () => tauriDriver.kill()
};
