// Example Tauri wdio spec: https://github.com/KittyCAD/modeling-app/blob/main/e2e/tauri/specs/app.spec.ts

console.log('\n\nRUNNING WDIO SPEC FILE\n\n');

describe('GitButler', () => {
	it('should render', async () => {
		const header = await $('body > h1');
		const text = await header.getText();
		expect(text).toMatch(/^[hH]ello/);
	});
});

console.log('\n\nDONE\n\n');
