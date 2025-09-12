import { getBaseURL, type GitButler, startGitButler } from '../src/setup.ts';
import {
	clickByTestId,
	fillByTestId,
	getByTestId,
	textEditorFillByTestId,
	waitForTestId,
	waitForTestIdToNotExist
} from '../src/util.ts';
import { expect, test } from '@playwright/test';
import { existsSync, readFileSync, writeFileSync } from 'fs';

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

test('should be able to apply a remote branch and integrate the remote changes - simple', async ({
	page,
	context
}, testInfo) => {
	const workdir = testInfo.outputPath('workdir');
	const configdir = testInfo.outputPath('config');
	gitbutler = await startGitButler(workdir, configdir, context);

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

	await gitbutler.runScript('project-with-remote-branches__add-commit-to-remote-branch.sh');

	// Click the sync button
	await clickByTestId(page, 'sync-button');

	// Integrate upstream commits
	await clickByTestId(page, 'upstream-commits-integrate-button');
	await waitForTestIdToNotExist(page, 'upstream-commits-integrate-button');
	await waitForTestIdToNotExist(page, 'upstream-commits-commit-action');

	const commitsAfterIntegration = getByTestId(page, 'commit-row');
	await expect(commitsAfterIntegration).toHaveCount(3);
});

test('should be able to apply a remote branch and integrate the remote changes - conflict', async ({
	page,
	context
}, testInfo) => {
	const workdir = testInfo.outputPath('workdir');
	const configdir = testInfo.outputPath('config');
	gitbutler = await startGitButler(workdir, configdir, context);

	const filePath = gitbutler.pathInWorkdir('local-clone/a_file');

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

	// Make a conflicting change
	writeFileSync(filePath, 'conflicting change\n', { flag: 'a' });

	// Commit the change
	await clickByTestId(page, 'start-commit-button');

	// Should see the commit drawer
	await waitForTestId(page, 'new-commit-view');

	const newCommitMessage = 'Conflicting change commit';
	const newCommitMessageBody = 'This should be oh-so-bad 🤭';
	// Write a commit message
	await fillByTestId(page, 'commit-drawer-title-input', newCommitMessage);
	await textEditorFillByTestId(page, 'commit-drawer-description-input', newCommitMessageBody);

	// Click the commit button
	await clickByTestId(page, 'commit-drawer-action-button');

	// This should create a new commit
	const commitsAfterChange = getByTestId(page, 'commit-row');
	await expect(commitsAfterChange).toHaveCount(3);

	await gitbutler.runScript('project-with-remote-branches__add-commit-to-remote-branch.sh');

	// Click the sync button
	await clickByTestId(page, 'sync-button');

	// Integrate upstream commits
	await clickByTestId(page, 'upstream-commits-integrate-button');
	await waitForTestIdToNotExist(page, 'upstream-commits-integrate-button');
	await waitForTestIdToNotExist(page, 'upstream-commits-commit-action');

	const commitsAfterIntegration = getByTestId(page, 'commit-row');
	await expect(commitsAfterIntegration).toHaveCount(4);

	const conflictedCommit = commitsAfterIntegration.filter({
		hasText: 'branch1: third commit'
	});
	await expect(conflictedCommit).toBeVisible();
	await conflictedCommit.click();

	// Click the resolve conflicts button
	await clickByTestId(page, 'commit-drawer-resolve-conflicts-button');

	// Should open the edit mode
	await waitForTestId(page, 'edit-mode');

	const conflictedFileContent = readFileSync(filePath, 'utf-8');
	expect(conflictedFileContent).toEqual(
		`foo
bar
baz
branch1 commit 1
branch1 commit 2
<<<<<` +
			`<< ours
conflicting change
||||||| ancestor
=======
branch1 commit 3
>>>>>>> theirs
`
	);

	// Resolve the conflict by keeping both changes
	writeFileSync(
		filePath,
		`foo
bar
baz
branch1 commit 1
branch1 commit 2
conflicting change
branch1 commit 3
`,
		{ flag: 'w' }
	);

	// Click the save and exit button
	await clickByTestId(page, 'edit-mode-save-and-exit-button');

	// Should be back in the workspace
	await waitForTestId(page, 'workspace-view');

	const commitsAfterResolving = getByTestId(page, 'commit-row');
	await expect(commitsAfterResolving).toHaveCount(4);

	const resolvedFileContent = readFileSync(filePath, 'utf-8');
	expect(resolvedFileContent).toEqual(
		`foo
bar
baz
branch1 commit 1
branch1 commit 2
conflicting change
branch1 commit 3
`
	);
});

test('should be able gracefully handle adding a branch that is ahead of our target commit', async ({
	page,
	context
}, testInfo) => {
	const workdir = testInfo.outputPath('workdir');
	const configdir = testInfo.outputPath('config');
	gitbutler = await startGitButler(workdir, configdir, context);

	const fileBPath = gitbutler.pathInWorkdir('local-clone/b_file');

	await gitbutler.runScript('project-with-remote-branches.sh');

	await page.goto('/');

	// Should load the workspace
	await waitForTestId(page, 'workspace-view');

	// There are remote changes in the base branch
	await gitbutler.runScript('project-with-remote-branches__add-commit-to-base-and-branch.sh');

	// Click the sync button
	await clickByTestId(page, 'sync-button');

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

	const commits = getByTestId(page, 'commit-row');
	// Should have 4 commits.
	// Three commits from branch 1, and the new commit from the base branch
	await expect(commits).toHaveCount(4);

	expect(existsSync(fileBPath)).toBe(true);
});

test('should be able gracefully handle adding a branch that is behind of our target commit', async ({
	page,
	context
}, testInfo) => {
	const workdir = testInfo.outputPath('workdir');
	const configdir = testInfo.outputPath('config');
	gitbutler = await startGitButler(workdir, configdir, context);

	const filePath = gitbutler.pathInWorkdir('local-clone/a_file');
	await gitbutler.runScript('project-with-remote-branches.sh');

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

	const commits = getByTestId(page, 'commit-row');
	await expect(commits).toHaveCount(2);

	const conflictedCommit = commits.filter({
		hasText: 'branch1: first commit'
	});
	await expect(conflictedCommit).toBeVisible();
	await conflictedCommit.click();

	// Click the resolve conflicts button
	await clickByTestId(page, 'commit-drawer-resolve-conflicts-button');

	// Should open the edit mode
	await waitForTestId(page, 'edit-mode');

	let conflictedFileContent = readFileSync(filePath, 'utf-8');
	expect(conflictedFileContent).toEqual(
		`foo
bar
baz
<<<<<` +
			`<< ours
Update to main branch
||||||| ancestor
=======
branch1 commit 1
>>>>>>> theirs
`
	);

	// Resolve the conflict by keeping both changes
	writeFileSync(
		filePath,
		`foo
bar
baz
Update to main branch
branch1 commit 1
`,
		{ flag: 'w' }
	);

	// Click the save and exit button
	await clickByTestId(page, 'edit-mode-save-and-exit-button');

	// Should be back in the workspace
	await waitForTestId(page, 'workspace-view');

	const commitsAfterResolving = getByTestId(page, 'commit-row');
	await expect(commitsAfterResolving).toHaveCount(2);

	// Verify the file content
	let resolvedFileContent = readFileSync(filePath, 'utf-8');
	expect(resolvedFileContent).toEqual(
		`foo
bar
baz
Update to main branch
branch1 commit 1
`
	);

	const commitsAfterResolution = getByTestId(page, 'commit-row');
	const conflictedCommitAfterResolution = commitsAfterResolution.filter({
		hasText: 'branch1: second commit'
	});
	await expect(conflictedCommitAfterResolution).toBeVisible();
	await conflictedCommitAfterResolution.click();

	// Click the resolve conflicts button
	await clickByTestId(page, 'commit-drawer-resolve-conflicts-button');

	// Should open the edit mode
	await waitForTestId(page, 'edit-mode');

	conflictedFileContent = readFileSync(filePath, 'utf-8');
	expect(conflictedFileContent).toEqual(
		`foo
bar
baz
Update to main branch
<<<<<` +
			`<< ours
branch1 commit 1
||||||| ancestor
=======
branch1 commit 2
>>>>>>> theirs
`
	);

	// Resolve the conflict by keeping both changes
	writeFileSync(
		filePath,
		`foo
bar
baz
Update to main branch
branch1 commit 1
branch1 commit 2
`,
		{ flag: 'w' }
	);

	// Click the save and exit button
	await clickByTestId(page, 'edit-mode-save-and-exit-button');

	// Should be back in the workspace
	await waitForTestId(page, 'workspace-view');

	// Verify the file content
	resolvedFileContent = readFileSync(filePath, 'utf-8');
	expect(resolvedFileContent).toEqual(
		`foo
bar
baz
Update to main branch
branch1 commit 1
branch1 commit 2
`
	);
});

test('should handle gracefully applying two conflicting branches', async ({
	page,
	context
}, testInfo) => {
	const workdir = testInfo.outputPath('workdir');
	const configdir = testInfo.outputPath('config');
	gitbutler = await startGitButler(workdir, configdir, context);

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

	const commits = getByTestId(page, 'commit-row');
	await expect(commits).toHaveCount(2);

	// Should navigate to the branches page when clicking the branches button
	await clickByTestId(page, 'navigation-branches-button');

	const branchCard2 = getByTestId(page, 'branch-list-card').filter({ hasText: 'branch2' });
	await expect(branchCard2).toBeVisible();
	await branchCard2.click();

	// Apply the second branch
	await clickByTestId(page, 'branches-view-apply-branch-button');
	// Should be redirected to the workspace
	await waitForTestId(page, 'workspace-view');

	// The modal explaining this should be visible
	await waitForTestId(page, 'stacks-unapplied-toast');
});
