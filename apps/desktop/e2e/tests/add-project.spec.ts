import { spawn } from 'node:child_process';
import { findAndClick, handleTelemetryPage } from '../utils.js';
import { browser } from '@wdio/globals';

describe('Project', () => {
	before(() => {
		// Use 'for-listing.sh' helper to generate dummy repositories for test
		spawn('bash', ['e2e/scripts/init-repositories.sh', '../../target/release/gitbutler-cli']);
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

		// 1. Click "Add local project"
		const addLocalProjectBtn = await $('div=Add local project');

		// For now, workaround by setting a file path in a new hidden input
		const filePathInput = await $('input[data-testid="test-directory-path"]');

		await browser.execute((s) => {
			s.value = '/opt/ndomino/home2021';
		}, filePathInput);

		await addLocalProjectBtn.click();

		// 2. Set target base branch
		const currentTargetBranchLabel = await $('h3=Target branch');
		await currentTargetBranchLabel.waitForDisplayed();

		if (await currentTargetBranchLabel.isExisting()) {
			// expect(currentTargetBranchLabel).toExist();

			const currentTargetBranchContinueBtn = await $('button=Continue');
			await currentTargetBranchContinueBtn.click();

			// 3. Git Authentication
			await $('h3=Git authentication');

			const gitAuthenticationContinueBtn = await $("button=Let's go!");
			await gitAuthenticationContinueBtn.click();
		}

		// 4. Board
		const workspaceButton = await $('button=Workspace');
		await expect(workspaceButton).toExist();
	});
});
