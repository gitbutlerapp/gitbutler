import { setElementValue, spawnAndLog, findAndClick } from '../utils.js';

describe('Project', () => {
	before(() => {
		spawnAndLog('bash', [
			'-c',
			'./e2e/scripts/init-repositories.sh ../../target/debug/gitbutler-cli'
		]);
	});

	it('should add a local project', async () => {
		await findAndClick('button=Continue');

		// Workaround selecting path via fileDialog by setting a hidden input value
		const dirInput = await $('input[data-testid="test-directory-path"]');
		setElementValue(dirInput, `${process.cwd()}/one-vbranch-on-integration`);

		await findAndClick('[data-testid="add-local-project"]');
		await findAndClick('button=Continue');
		await findAndClick("button=Let's go!");
		const workspaceButton = await $('button=Workspace');
		console.log('WORKSPACE.BUTTON', workspaceButton);
		await expect(workspaceButton).toExist();
	});
});
