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
	// set timeout to 2 minutes to cover build
	this.timeout(60000 * 2);

	// For CI(?) - ensure the program has been built
	// spawnSync("cargo", ["build", "--release"]);

	tauriDriver = spawn(path.resolve(os.homedir(), '.cargo', 'bin', 'tauri-driver'), [], {
		stdio: [null, process.stdout, process.stderr]
	});

	const capabilities = new Capabilities();
	capabilities.set('tauri:options', { application });
	capabilities.setBrowserName('');

	driver = await new Builder()
		.withCapabilities(capabilities)
		.usingServer('http://localhost:4444/')
		.build();
});

after(async function () {
	console.log('\n\nCALLING AFTER CLEANING FNs');

	// stop the webdriver session
	await driver?.quit();

	// kill the tauri-driver process
	tauriDriver?.kill();
});

console.log('\n\nSTARTING TEST');

describe('GitButler Startup', () => {
	console.log('\n\nSTARTING DESCRIBE');

	it('should have gray background', async () => {
		console.log('\n\nIT_SHOULD_HAVE_GRAY_BACKGROUND');

		const text = await driver.findElement(By.css('body')).getCssValue('background-color');
		expect(text).to.match(/#000/);
	});

	console.log('\n\nFINISHING DSECRIBE');
});

console.log('\n\nFINISHING TEST');
