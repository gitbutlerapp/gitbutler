import { getBaseURL, type GitButler, startGitButler } from '../src/setup.ts';
import { clickByTestId, dragAndDropByLocator, getByTestId, waitForTestId } from '../src/util.ts';
import { expect, test } from '@playwright/test';
import { writeFileSync } from 'fs';

let gitbutler: GitButler;

test.use({
	baseURL: getBaseURL()
});

test.afterEach(async () => {
	await gitbutler.destroy();
});

test('should be able to amend a file to a commit', async ({ page, context }, testInfo) => {
	const workdir = testInfo.outputPath('workdir');
	const configdir = testInfo.outputPath('config');
	gitbutler = await startGitButler(workdir, configdir, context);

	const filePath = gitbutler.pathInWorkdir('local-clone/b_file');

	await gitbutler.runScript('project-with-remote-branches.sh');

	await page.goto('/');

	// Should load the workspace
	await waitForTestId(page, 'workspace-view');

	// Should navigate to the branches page when clicking the branches button
	await clickByTestId(page, 'navigation-branches-button');
	const header = await waitForTestId(page, 'target-commit-list-header');

	await expect(header).toContainText('origin/master');

	const branchListCards = getByTestId(page, 'branch-list-card');
	await expect(branchListCards).toHaveCount(3);

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

	// Push the changes to the remote branch
	// (it's basically a no-op, just makes sure that the same commits after rebasing are on the remote)
	await clickByTestId(page, 'stack-push-button');

	// Add a new file
	writeFileSync(filePath, 'Hello! this is file b\n', { flag: 'w' });

	const fileLocator = getByTestId(page, 'file-list-item').filter({ hasText: 'b_file' });
	const topCommitLocator = getByTestId(page, 'commit-row').filter({
		hasText: 'branch1:  second commit'
	});

	// Drag the new file onto the top commit, to amend it
	await dragAndDropByLocator(page, fileLocator, topCommitLocator);

	// Push the changes to the remote branch
	await clickByTestId(page, 'stack-push-button');

	// The stack should have two commits
	const commitsAfterAmend = getByTestId(page, 'commit-row');
	await expect(commitsAfterAmend).toHaveCount(2);

	const pushButton = getByTestId(page, 'stack-push-button');
	await expect(pushButton).toBeDisabled();
});
