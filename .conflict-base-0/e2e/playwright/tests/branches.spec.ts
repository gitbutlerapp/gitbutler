import { getBaseURL, type GitButler, startGitButler } from '../src/setup.ts';
import { clickByTestId, getByTestId, waitForTestId } from '../src/util.ts';
import { expect, test } from '@playwright/test';

let gitbutler: GitButler;

test.use({
	baseURL: getBaseURL()
});

test.afterEach(async () => {
	gitbutler?.destroy();
});

test('should be able to apply a remote branch', async ({ page, context }, testInfo) => {
	const workdir = testInfo.outputPath('workdir');
	const configdir = testInfo.outputPath('config');
	gitbutler = await startGitButler(workdir, configdir, context);

	// const projectPath = gitbutler.pathInWorkdir('local-clone/');

	await gitbutler.runScript('project-with-remote-branches.sh');

	await page.goto('/');

	// Should load the workspace
	await waitForTestId(page, 'workspace-view');

	// Should navigate to the branches page when clicking the branches button
	await clickByTestId(page, 'navigation-branches-button');
	const header = await waitForTestId(page, 'target-commit-list-header');

	await expect(header).toContainText('origin/master');

	const branchListCards = getByTestId(page, 'branch-list-card');
	await expect(branchListCards).toHaveCount(2);

	const firstBranchCard = branchListCards.filter({ hasText: 'branch1' });
	await expect(firstBranchCard).toBeVisible();
	await firstBranchCard.click();

	// The delete branch should be visible
	await waitForTestId(page, 'branches-view-delete-local-branch-button');

	// Apply the branch
	await clickByTestId(page, 'branches-view-apply-branch-button');
	// Should be redirected to the workspace
	await waitForTestId(page, 'workspace-view');

	// There should be only one stack
	const stacks = getByTestId(page, 'stack');
	await expect(stacks).toHaveCount(1);
	const stack = stacks.first();
	await expect(stack).toContainText('branch1');

	// The stack should have two commits
	const commits = getByTestId(page, 'commit-row');
	await expect(commits).toHaveCount(2);
});
