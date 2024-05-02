import os from 'node:os';
import path from 'node:path';
import { spawn, spawnSync, ChildProcess } from 'node:child_process';

let tauriDriver: ChildProcess;

const application = process.env.E2E_APPLICATION || `../target/release/gitbutler-tauri`;

export const config = {
	port: 4444,
	specs: ['./e2e/wdio/**/*.js'],
	maxInstances: 1,
	capabilities: [
		{
			maxInstances: 1,
			browserName: 'wry',
			'tauri:options': {
				application
			}
		}
	],
	reporters: ['spec'],
	framework: 'mocha',
	mochaOpts: {
		ui: 'bdd',
		timeout: 600000
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
