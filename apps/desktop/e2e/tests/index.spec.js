describe('GitButler', () => {
	it('should have the root element', async () => {
		const element = await $('body.text-base');
		await expect(element).toExist();
	});
});
