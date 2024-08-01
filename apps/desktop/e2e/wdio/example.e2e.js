// Example Tauri wdio spec: https://github.com/KittyCAD/modeling-app/blob/main/e2e/tauri/specs/app.spec.ts

console.log('\n\nWDIO - START OF TEST FILE \n\n');

describe('GitButler', () => {
	console.log('\n\nWDIO - INSIDE DESCRIBE() \n\n');

	it('should have the root element', async () => {
		console.log('\n\nWDIO - INSIDE IT() \n\n');
		const element = await $('body.text-base');

		console.log('\n\nWDIO - SHOULD.HAVE ROOT\n\n');
		expect(element).toExist();
	});
});

console.log('\n\nWDIO - DONE\n\n');
