import { spawnAndLog, findAndClick, setElementValue } from '../utils.js';

describe('Project', () => {
	before(() => {
		spawnAndLog('bash', [
			'-c',
			'./e2e/scripts/init-repositories.sh ../../target/debug/gitbutler-cli'
		]);
	});

	it('should add a local project', async () => {
		await findAndClick('button[data-testid="analytics-continue"]');

		const dirInput = await $('input[data-testid="test-directory-path"]');
		await setElementValue(dirInput, '/tmp/gb-e2e-repos/one-vbranch-on-integration');

		await findAndClick('button[data-testid="add-local-project"]');
		// TODO: Remove next click when v3 is default!
		await findAndClick('button[data-testid="set-base-branch"]');
		await findAndClick('button[data-testid="accept-git-auth"]');

		const workspaceButton = await $('button[data-testid="navigation-workspace-button"]');
		await expect(workspaceButton).toExist();
	});
});
