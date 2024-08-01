import { spawn } from 'node:child_process';
import os from 'node:os';
import path from 'node:path';
// import { spawn, spawnSync } from 'node:child_process';

// keep track of the `tauri-driver` child process
let tauriDriver;

export const config = {
	hostname: 'localhost',
	port: 4444,
	specs: ['./e2e/wdio/**/*.js'],
	maxInstances: 1,
	capabilities: [
		{
			maxInstances: 1,
			'tauri:options': {
				application: '../../target/release/git-butler-dev',
				webviewOptions: {} // Windows only
			}
		}
	],
	reporters: ['spec'],
	framework: 'mocha',
	mochaOpts: {
		bail: true,
		ui: 'bdd',
		timeout: 60000
	},

	// ensure the rust project is built since we expect this binary to exist for the webdriver sessions
	// onPrepare: () => spawnSync('cargo', ['build', '--release']),

	// ensure we are running `tauri-driver` before the session starts so that we can proxy the webdriver requests
	beforeSession: () =>
		(tauriDriver = spawn(path.resolve(os.homedir(), '.cargo', 'bin', 'tauri-driver'), [], {
			stdio: [null, process.stdout, process.stderr]
		})),

	// clean up the `tauri-driver` process we spawned at the start of the session
	afterSession: () => tauriDriver.kill()
};
