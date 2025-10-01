import { getBaseURL, type GitButler, startGitButler } from '../src/setup.ts';
import { clickByTestId, getByTestId, waitForTestId, waitForTestIdToNotExist } from '../src/util.ts';
import { expect, test } from '@playwright/test';

let gitbutler: GitButler;

test.use({
	baseURL: getBaseURL()
});

test.afterEach(async () => {
	gitbutler?.destroy();
});

test('should handle the update of workspace with one conflicting branch gracefully', async ({
	page,
	context
}, testInfo) => {
	const workdir = testInfo.outputPath('workdir');
	const configdir = testInfo.outputPath('config');
	gitbutler = await startGitButler(workdir, configdir, context);

	await gitbutler.runScript('project-with-remote-branches.sh');
	// Apply branch1
	await gitbutler.runScript('apply-upstream-branch.sh', ['branch1', 'local-clone']);

	await page.goto('/');

	// Should load the workspace
	await waitForTestId(page, 'workspace-view');

	// There are remote changes in the base branch
	await gitbutler.runScript('project-with-remote-branches__add-commit-to-base.sh');

	// Click the sync button
	await clickByTestId(page, 'sync-button');

	// Update the workspace
	await clickByTestId(page, 'integrate-upstream-commits-button');
	await clickByTestId(page, 'integrate-upstream-action-button');

	// There should be one stack
	const stacks = getByTestId(page, 'stack');
	await expect(stacks).toHaveCount(1);
});

test('should handle the update of workspace with integrated branch gracefully', async ({
	page,
	context
}, testInfo) => {
	const workdir = testInfo.outputPath('workdir');
	const configdir = testInfo.outputPath('config');
	gitbutler = await startGitButler(workdir, configdir, context);

	await gitbutler.runScript('project-with-remote-branches.sh');
	// Apply branch1
	await gitbutler.runScript('apply-upstream-branch.sh', ['branch1', 'local-clone']);

	await page.goto('/');

	// Should load the workspace
	await waitForTestId(page, 'workspace-view');

	// There should be one stack applied
	const stacks = getByTestId(page, 'stack');
	await expect(stacks).toHaveCount(1);

	// Branch one was merged in the forge
	await gitbutler.runScript('merge-upstream-branch-to-base.sh', ['branch1']);

	// Click the sync button
	await clickByTestId(page, 'sync-button');

	// Update the workspace
	await clickByTestId(page, 'integrate-upstream-commits-button');
	await clickByTestId(page, 'integrate-upstream-action-button');

	// There should be no stacks
	await waitForTestIdToNotExist(page, 'stack');
});

test('should handle the update of the workspace with multiple stacks gracefully', async ({
	page,
	context
}, testInfo) => {
	const workdir = testInfo.outputPath('workdir');
	const configdir = testInfo.outputPath('config');
	gitbutler = await startGitButler(workdir, configdir, context);

	await gitbutler.runScript('project-with-stacks.sh');
	await gitbutler.runScript('apply-upstream-branch.sh', ['branch1', 'local-clone']);
	await gitbutler.runScript('apply-upstream-branch.sh', ['branch2', 'local-clone']);

	await page.goto('/');

	// Should load the workspace
	await waitForTestId(page, 'workspace-view');

	// There should be one stack applied
	let stacks = getByTestId(page, 'stack');
	await expect(stacks).toHaveCount(2);

	// Branch one was merged in the forge
	await gitbutler.runScript('merge-upstream-branch-to-base.sh', ['branch1']);

	// Click the sync button
	await clickByTestId(page, 'sync-button');

	// Update the workspace
	await clickByTestId(page, 'integrate-upstream-commits-button');
	await clickByTestId(page, 'integrate-upstream-action-button');

	// There should be one stack left
	stacks = getByTestId(page, 'stack');
	await expect(stacks).toHaveCount(1);
});
