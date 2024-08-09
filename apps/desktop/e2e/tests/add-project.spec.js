import { spawn } from 'node:child_process';
import { browser } from '@wdio/globals';

describe('Project', () => {
	before(() => {
		// Use 'for-listing.sh' helper to generate dummy repositories for test
		spawn('bash', ['e2e/scripts/init-repositories.sh', '../../target/release/gitbutler-cli']);
	});

	it('should add a local project', async () => {
		// 0. Accept Telemetry
		// TODO: Fix broken import in wdio
		// await handleTelemetryPage();
		const telemetryAgreement = await $('h1=Before we begin');
		await telemetryAgreement.waitForDisplayed();

		const acceptTelemetryBtn = await $('button=Continue');
		// const acceptTelemetryBtn = await $('button[data-testid="analytics-continue"]');
		await acceptTelemetryBtn.click();

		// 1. Click "Add local project"
		const addLocalProjectBtn = await $('div=Add local project');

		// For now, workaround by setting a file path in a new hidden input
		const filePathInput = await $('input[data-testid="test-directory-path"]');

		browser.execute((s) => {
			s.value = './one-vbranch-on-integration';
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
