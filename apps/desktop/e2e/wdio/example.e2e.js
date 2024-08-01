// Example Tauri wdio spec: https://github.com/KittyCAD/modeling-app/blob/main/e2e/tauri/specs/app.spec.ts

console.log('\n\nRUNNING WDIO SPEC FILE\n\n');

describe('GitButler', () => {
	it('should have the root element', async () => {
		console.log('\n\nSHOULD.HAVE ROOT\n\n');
		const element = await $('body.text-base');
		expect(element).toExist();
	});
});

console.log('\n\nDONE\n\n');
