import { spawn } from 'node:child_process';
import path from 'node:path';
import os from 'node:os';
import { test, expect } from '@playwright/test';
import { Builder, By, Capabilities } from 'selenium-webdriver';

type TODO = any;

const application = path.resolve(
	import.meta.dirname,
	'..',
	'..',
	'..',
	'target',
	'release',
	'git-butler-dev'
);

// keep track of the webdriver instance we create
let driver: TODO;

// keep track of the tauri-driver process we start
let tauriDriver: TODO;

test.beforeAll(async () => {
	// // set timeout to 2 minutes to allow the program to build if it needs to
	// this.timeout(120000);

	// ensure the program has been built
	// spawnSync('cargo', ['build', '--release']);

	// start tauri-driver
	tauriDriver = spawn(path.resolve(os.homedir(), '.cargo', 'bin', 'tauri-driver'), [], {
		stdio: [null, process.stdout, process.stderr]
	});
	tauriDriver?.stdout?.on('data', (data: TODO) => {
		console.log(`stdout: ${data}`);
	});

	const capabilities = new Capabilities();
	capabilities.set('tauri:options', { application });
	capabilities.setBrowserName('wry');
	capabilities.setAcceptInsecureCerts(true);
	capabilities.setPlatform('tauri');
	capabilities.set('unhandledPromptBehavior', 'ignore');

	// TODO: "Failed to match capabilities" response from WebKitWebDriver
	// See: https://github.com/WebKit/WebKit/blob/main/Source/WebDriver/WebDriverService.cpp#L794

	// start the webdriver client
	driver = await new Builder()
		.withCapabilities(capabilities)
		.usingServer('http://localhost:4444/')
		.build();

	console.log('driver', driver);
});

test('driver color test', async () => {
	// selenium returns color css values as rgb(r, g, b)
	const text = await driver.findElement(By.css('body')).getCssValue('background-color');

	expect(text).toBeTruthy();
});

// test('has empty title', async ({ page }) => {
// 	console.log('has empty title');
// 	await page.goto('http://localhost:1420');
//
// 	await expect(page).toHaveTitle('');
// });
//
// test('has text package.json', async ({ page }) => {
// 	await page.goto('http://localhost:1420');
//
// 	const listBox = page.getByRole('listbox').getByRole('button').first();
//
// 	await expect(listBox).toHaveText('package.json');
// });

test.afterAll(async function () {
	// stop the webdriver session
	await driver?.quit();

	// kill the tauri-driver process
	tauriDriver?.kill();
});
