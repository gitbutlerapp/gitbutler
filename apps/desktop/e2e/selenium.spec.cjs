const os = require('os');
const path = require('path');
const { spawn } = require('child_process');
const { Builder, By, Capabilities } = require('selenium-webdriver');

// See: https://tauri.app/v1/guides/testing/webdriver/example/selenium/

const application = path.resolve(
	__dirname,
	'..',
	'..',
	'..',
	'target',
	'release',
	'git-butler-dev'
);

let driver;
let tauriDriver;

before(async function () {
	// set timeout to 2 minutes
	this.timeout(120000);

	tauriDriver = spawn(path.resolve(os.homedir(), '.cargo', 'bin', 'tauri-driver')); //, [], {
	// 	stdio: [null, process.stdout, process.stderr]
	// });

	tauriDriver?.stdout?.on('data', (data) => {
		console.log(data.toString());
	});

	const capabilities = new Capabilities();
	capabilities.set('tauri:options', { application });
	capabilities.setBrowserName('wry');

	driver = await new Builder()
		.withCapabilities(capabilities)
		.usingServer('http://localhost:4444')
		.build();
});

describe('Hello Tauri', () => {
	it('should be cordial', async () => {
		const text = await driver.findElement(By.css('body > h1')).getText();
		expect(text).to.match(/^[hH]ello/);
	});
});

after(async function () {
	// stop the webdriver session
	await driver?.quit();

	// kill the tauri-driver process
	tauriDriver.kill();
});
