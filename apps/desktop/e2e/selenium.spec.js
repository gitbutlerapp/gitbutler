import os from 'node:os';
import path from 'node:path';
import { spawn } from 'node:child_process';
import { Builder, By, Capabilities } from 'selenium-webdriver';
import { expect } from 'chai'

// See: https://tauri.app/v1/guides/testing/webdriver/example/selenium/

// const application = path.resolve(
// 	import.meta.dirname,
// 	'..',
// 	'..',
// 	'..',
// 	'target',
// 	'release',
// 	'git-butler-dev'
// );

let driver;
let tauriDriver;

before(async function () {
	// // set timeout to 2 minutes to cover build
	// this.timeout(60000 * 2);

	// For CI(?) - ensure the program has been built
	// spawnSync("cargo", ["build", "--release"]);

	tauriDriver = spawn(path.resolve(os.homedir(), '.cargo', 'bin', 'tauri-driver'), [], {
		stdio: [null, process.stdout, process.stderr]
	});

	const capabilities = new Capabilities();
	capabilities.set('tauri:options', { application: '../../target/release/git-butler-dev' });
	capabilities.setBrowserName('wry'); // Setting this to 'wry' triggers the "Capabilities do not match" error

	driver = await new Builder()
		.withCapabilities(capabilities)
		.usingServer('http://localhost:4444/')
		.build();
});

after(async function () {
	// stop the webdriver session
	await driver?.quit();

	// kill the tauri-driver process
	tauriDriver?.kill();
});

console.log('\nSTARTING TEST');

describe('GitButler Startup', () => {
	console.log('\nSTARTING DESCRIBE');

	it('should have gray background', async () => {
		console.log('\nIT_SHOULD_HAVE_GRAY_BACKGROUND');

		const text = await driver.findElement(By.css('body.text-base'))
		expect(text).to.exist
	});

	console.log('\nFINISHING DSECRIBE');
});

console.log('\nFINISHING TEST');
