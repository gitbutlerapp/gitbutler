import { spawnSync } from 'node:child_process';
import { findAndClick } from '../utils.js';
import { browser } from '@wdio/globals';

describe('Project', () => {
	before(() => {
		// Use 'for-listing.sh' helper to generate dummy repositories for test
		console.log('Initializing test repo(s)');
		const result = spawnSync('bash', [
			'-c',
			'e2e/scripts/init-repositories.sh ../../target/debug/gitbutler-cli'
		]);
		console.log('stdout: ', result.stdout?.toString());
		console.log('stderr: ', result.stderr?.toString());
		console.log('status: ', result.status);
	});

	it('should add a local project', async () => {
		// 0. Accept Telemetry
		// await handleTelemetryPage();
		// const telemetryAgreement = await $('h1=Before we begin');
		// await telemetryAgreement.waitForDisplayed();
		//
		// const acceptTelemetryBtn = await $('button=Continue');
		// // const acceptTelemetryBtn = await $('button[data-testid="analytics-continue"]');
		// await acceptTelemetryBtn.click();
		await findAndClick('button=Continue');

		// For now, workaround by setting a file path in a new hidden input
		const dirInput = await $('input[data-testid="test-directory-path"]');

		await browser.execute(
			(input, path) => {
				(input as any).value = path;
			},
			dirInput,
			process.cwd() + '/one-vbranch-on-integration'
		);

		await findAndClick('[data-testid="add-local-project"]');
		await findAndClick('button=Continue');
		await findAndClick("button=Let's go!");
		const workspaceButton = await $('button=Workspace');
		await expect(workspaceButton).toExist();
	});
});
