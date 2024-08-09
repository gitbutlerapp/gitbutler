import os from 'node:os';
import path from 'node:path';
import { statSync } from 'node:fs';
import { spawn, spawnSync } from 'node:child_process';
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

	// Run `init-repositories.sh` if necessary
	if (!statSync(path.resolve('./one-vbranch-on-integration'), { throwIfNoEntry: false })) {
		spawn('bash', ['e2e/scripts/init-repositories.sh', '../../target/release/gitbutler-cli']);
	}

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

describe('GitButler', function () {
	it('should have body', async function () {
		const text = await driver.findElement(By.css('body.text-base'));
		expect(text).to.exist;
	});
});

describe('On-Boarding', function () {
	this.timeout(20000);
	it('should add a local project', async function () {
		await driver.sleep(1000);

		// 0. Accept Telemetry Warnings
		const telemetryAgreementShown = await driver.findElement(By.css('h1'));
		await driver.wait(until.elementIsVisible(telemetryAgreementShown), 10000);

		const acceptTelemetryBtn = await driver.findElement(
			By.css('button[data-testid="analytics-continue"]')
		);
		await acceptTelemetryBtn.click();

		// 1. Add Local Project
		const addLocalProjectBtn = await driver.findElement(
			By.xpath('.//div[normalize-space(text()) = "Add local project"]')
		);

		// 2. Set input value to repository path
		const targetRepositoryPath = './one-vbranch-on-integration';
		const filePathInput = await driver.findElement(
			By.css('input[data-testid="test-directory-path"]')
		);

		expect(filePathInput).to.exist;
		driver.executeScript(
			"arguments[0].setAttribute('value',arguments[1])",
			filePathInput,
			targetRepositoryPath
		);
		await addLocalProjectBtn.click();

		await driver.sleep(2000);

		// 3. Accept Base Branch
		const baseBranchLabel = await driver.findElement(
			By.xpath(
				'.//h3[normalize-space(text()) = "Target branch"] | .//h3[not(.//h3[normalize-space(text()) = "Target branch"]) and normalize-space() = "Target branch"]'
			)
		);

		expect(baseBranchLabel).to.exist;
		const baseBranchContinueBtn = await driver.findElement(
			By.xpath(
				'.//button[normalize-space(text()) = "Continue"] | .//button[not(.//button[normalize-space(text()) = "Continue"]) and normalize-space() = "Continue"]'
			)
		);
		await baseBranchContinueBtn.click();

		await driver.sleep(2000);

		// 4. Git Authentication
		const gitAuthenticationBtn = await driver.findElement(
			By.xpath(
				'.//button[normalize-space(text()) = "Let\'s go!"] | .//button[not(.//button[normalize-space(text()) = "Let\'s go!"]) and normalize-space() = "Let\'s go!"]'
			)
		);
		await gitAuthenticationBtn.click();

		await driver.sleep(5000);

		// 5. Project Board
		const workspaceBtn = await driver.findElement(
			By.xpath(
				'.//button[normalize-space(text()) = "Workspace"] | .//button[not(.//button[normalize-space(text()) = "Workspace"]) and normalize-space() = "Workspace"]'
			)
		);

		expect(workspaceBtn).to.exist;

		await driver.sleep(2000);
	});
});
