import { spawnAndLog, findAndClick } from '../utils.js';
import { browser } from '@wdio/globals';

describe('Project', () => {
	before(() => {
		spawnAndLog('bash', [
			'-c',
			'./e2e/scripts/init-repositories.sh ../../target/debug/gitbutler-cli'
		]);
	});

	it('should add a local project', async () => {
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
