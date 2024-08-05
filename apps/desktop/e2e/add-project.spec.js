// import { handleTelemetryPage } from './utils';

describe('Project', () => {
	it('should add a local project', async () => {
		// TODO: Fix broken import in wdio :shrug:
		// await handleTelemetryPage();
		const telemetryAgreement = await $('div=GitButler uses telemetry');
		if (await telemetryAgreement.isExisting()) {
			const acceptTelemetryBtn = await $('button=Continue');
			await acceptTelemetryBtn.click();
		}

		const addLocalProjectBtn = await $('div=Add local project');
		expect(addLocalProjectBtn).toExist();

		// Now a FileDialog pops up, maybe we can control it by sending in keyboard commands?
		// await addLocalProjectBtn.click();

		// For now, hacky workaround by setting a file path in a new hidden input
		const filePathInput = await $('input#test-directory-path');
		expect(filePathInput).toExist();
		filePathInput.setValue('/opt/ndomino/home2021');

		await addLocalProjectBtn.click();
	});
});
