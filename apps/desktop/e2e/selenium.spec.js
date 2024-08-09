import os from 'node:os';
import path from 'node:path';
import { spawn } from 'node:child_process';
import { Builder, By, until, Capabilities } from 'selenium-webdriver';
import { expect } from 'chai';

// See: https://tauri.app/v1/guides/testing/webdriver/example/selenium/

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

describe('GitButler', function () {
	it('should have body', async function () {
		const text = await driver.findElement(By.css('body.text-base'));
		expect(text).to.exist;
	});
});

describe('On-Boarding', function () {
	this.timeout(20000);
	it('should add a local project', async function () {
		// await driver.manage().setTimeouts({ explicit: 20000, implicit: 20000 });
		await driver.sleep(2000);

		const telemetryAgreementShown = await driver.findElement(By.css('h1'));
		await driver.wait(until.elementIsVisible(telemetryAgreementShown), 10000);

		const acceptTelemetryBtn = await driver.findElement(By.css('button'));
		// By.css('button[data-testid="analytics-continue"]')
		await acceptTelemetryBtn.click();
	});
});
