describe('Project', () => {
	it('should add a local project', async () => {
		const addLocalProjectBtn = await $('div=Add local project');
		expect(addLocalProjectBtn).toExist();

		// Now a FileDialog pops up, maybe we can control it by sending in keyboard commands?
		// await addLocalProjectBtn.click();

		// For now, hacky workaround by setting a file path in a new hidden input
		const filePathInput = await $('input#test-directory-path');
		filePathInput.value = '/opt/ndomino/home2021';
	});
});
