// import { handleTelemetryPage } from './utils';
import { browser } from '@wdio/globals';

describe('Project', () => {
	it('should add a local project', async () => {
		// 0. Accept Telemetry
		// TODO: Fix broken import in wdio :shrug:
		// await handleTelemetryPage();
		const telemetryAgreement = await $('h1=Before we begin');
		await telemetryAgreement.waitForDisplayed();

		const acceptTelemetryBtn = await $('button[data-testid="analytics-continue"]');
		await acceptTelemetryBtn.click();

		// 1. Click "Add local project"
		const addLocalProjectBtn = await $('div=Add local project');
		expect(addLocalProjectBtn).toExist();

		// For now, workaround by setting a file path in a new hidden input
		const filePathInput = await $('input[data-testid="test-directory-path"]');
		expect(filePathInput).toExist();
		browser.execute((s) => {
			s.value = '/opt/ndomino/home2021';
		}, filePathInput);
		await addLocalProjectBtn.click();

		// 2. Set target base branch
		const currentTargetBranchLabel = await $('h3=Target branch');

		if (await currentTargetBranchLabel.isExisting()) {
			// expect(currentTargetBranchLabel).toExist();

			const currentTargetBranchContinueBtn = await $('button=Continue');
			await currentTargetBranchContinueBtn.click();

			// 3. Git Authentication
			const gitAuthenticationLabel = await $('h3=Git authentication');
			expect(gitAuthenticationLabel).toExist();

			const gitAuthenticationContinueBtn = await $("button=Let's go!");
			await gitAuthenticationContinueBtn.click();
		}

		// 4. Board
		const boardWorkspaceBtn = await $('button=Workspace');
		expect(boardWorkspaceBtn).toExist();
	});
});
