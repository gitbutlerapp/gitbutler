import { spawnAndLog, findAndClick, setElementValue } from '../utils.js';

describe('Project', () => {
	before(() => {
		spawnAndLog('bash', [
			'-c',
			'./blackbox/scripts/init-repositories.sh ../target/debug/gitbutler-cli'
		]);
	});

	it('should add a local project', async () => {
		await findAndClick('button[data-testid="analytics-continue"]');

		const dirInputSelection = $('input[data-testid="test-directory-path"]');
		const dirInput = await dirInputSelection.getElement();
		await setElementValue(dirInput, '/tmp/gb-e2e-repos/one-vbranch-on-integration');

		await findAndClick('button[data-testid="add-local-project"]');
		// TODO: Remove next click when v3 is default!
		await findAndClick('button[data-testid="set-base-branch"]');

		const workspaceButton = await $(
			'button[data-testid="navigation-workspace-button"]'
		).getElement();

		await expect(workspaceButton).toExist();
	});
});
