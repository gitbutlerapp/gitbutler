import { getBaseURL, type GitButler, startGitButler } from '../src/setup.ts';
import { clickByTestId, getByTestId, waitForTestId, waitForTestIdToNotExist } from '../src/util.ts';
import { expect, test } from '@playwright/test';

let gitbutler: GitButler;

test.use({
	baseURL: getBaseURL()
});

test.afterEach(async () => {
	await gitbutler?.destroy();
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

test('should handle the update of workspace with integrated parent branch in stack gracefully', async ({
	page,
	context
}, testInfo) => {
	const workdir = testInfo.outputPath('workdir');
	const configdir = testInfo.outputPath('config');
	gitbutler = await startGitButler(workdir, configdir, context);

	await gitbutler.runScript('project-with-remote-branches.sh');
	// Apply branch1
	await gitbutler.runScript('apply-upstream-branch.sh', ['branch1', 'local-clone']);
	await gitbutler.runScript('apply-upstream-branch.sh', ['branch3', 'local-clone']);
	await gitbutler.runScript('move-branch.sh', ['branch3', 'branch1', 'local-clone']);

	await page.goto('/');

	// Should load the workspace
	await waitForTestId(page, 'workspace-view');

	// There should be one stack applied
	let stacks = getByTestId(page, 'stack');
	await expect(stacks).toHaveCount(1);
	let branchCards = getByTestId(page, 'branch-card');
	await expect(branchCards).toHaveCount(2);

	// Branch one was merged in the forge
	await gitbutler.runScript('merge-upstream-branch-to-base.sh', ['branch1']);

	// Click the sync button
	await clickByTestId(page, 'sync-button');

	// Update the workspace
	await clickByTestId(page, 'integrate-upstream-commits-button');

	// The staus of the branch1 should be "Integrated"
	const branch1Status = page.locator('[data-integration-row-branch-name="branch1"]').first();
	await branch1Status.waitFor();
	const statusBadge = branch1Status.getByTestId('integrate-upstream-series-row-status-badge');
	await statusBadge.waitFor();
	await expect(statusBadge).toHaveText('Integrated');

	await clickByTestId(page, 'integrate-upstream-action-button');

	// There should be one stack left with one branch
	stacks = getByTestId(page, 'stack');
	await expect(stacks).toHaveCount(1);
	branchCards = getByTestId(page, 'branch-card');
	await expect(branchCards).toHaveCount(1);
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

test('should handle the update of an empty branch gracefully', async ({
	page,
	context
}, testInfo) => {
	const workdir = testInfo.outputPath('workdir');
	const configdir = testInfo.outputPath('config');
	gitbutler = await startGitButler(workdir, configdir, context);

	await gitbutler.runScript('project-with-stacks.sh');

	await page.goto('/');

	// Should load the workspace
	await waitForTestId(page, 'workspace-view');

	// There should be no stacks
	let stacks = getByTestId(page, 'stack');
	await expect(stacks).toHaveCount(0);

	// Create a new branch
	await clickByTestId(page, 'chrome-create-new-button');
	await clickByTestId(page, 'chrome-header-create-branch-menu-item');
	const modal = await waitForTestId(page, 'create-new-branch-modal');

	const input = modal.locator('#new-branch-name-input');
	await input.fill('new-branch');
	await clickByTestId(page, 'confirm-submit');

	// There should be no stacks
	stacks = getByTestId(page, 'stack');
	await expect(stacks).toHaveCount(1);

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

test('should handle the update of a branch with an empty commit', async ({
	page,
	context
}, testInfo) => {
	const workdir = testInfo.outputPath('workdir');
	const configdir = testInfo.outputPath('config');
	gitbutler = await startGitButler(workdir, configdir, context);

	await gitbutler.runScript('project-with-stacks.sh');

	await page.goto('/');

	// Should load the workspace
	await waitForTestId(page, 'workspace-view');

	// There should be no stacks
	let stacks = getByTestId(page, 'stack');
	await expect(stacks).toHaveCount(0);

	// Create a new branch
	await clickByTestId(page, 'chrome-create-new-button');
	await clickByTestId(page, 'chrome-header-create-branch-menu-item');
	const modal = await waitForTestId(page, 'create-new-branch-modal');

	const input = modal.locator('#new-branch-name-input');
	await input.fill('new-branch');
	await clickByTestId(page, 'confirm-submit');

	// There should be one stack
	stacks = getByTestId(page, 'stack');
	await expect(stacks).toHaveCount(1);

	const branchCard = getByTestId(page, 'branch-card');
	await branchCard.isVisible();
	await branchCard.click({
		button: 'right'
	});

	// Add an empty commit
	await waitForTestId(page, 'branch-header-context-menu');
	await clickByTestId(page, 'branch-header-context-menu-add-empty-commit');

	// There should be one commit
	let commits = getByTestId(page, 'commit-row');
	await expect(commits).toHaveCount(1);

	// Branch one was merged in the forge
	await gitbutler.runScript('merge-upstream-branch-to-base.sh', ['branch1']);

	// Click the sync button
	await clickByTestId(page, 'sync-button');

	// Update the workspace
	await clickByTestId(page, 'integrate-upstream-commits-button');
	await clickByTestId(page, 'integrate-upstream-action-button');

	await waitForTestIdToNotExist(page, 'integrate-upstream-action-button');

	// There should be one stack left
	stacks = getByTestId(page, 'stack');
	await expect(stacks).toHaveCount(1);
	// There should be one commit
	commits = getByTestId(page, 'commit-row');
	await expect(commits).toHaveCount(1);
});
