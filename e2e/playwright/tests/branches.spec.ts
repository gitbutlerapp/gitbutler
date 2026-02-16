import { createNewBranch, deleteBranch, unapplyStack } from '../src/branch.ts';
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
	await gitbutler?.destroy();
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
	const header = await waitForTestId(page, 'branch-header');

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
	const header = await waitForTestId(page, 'branch-header');

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

test('should be able to apply a remote branch and integrate the remote changes - create commit', async ({
	page,
	context
}, testInfo) => {
	const workdir = testInfo.outputPath('workdir');
	const configdir = testInfo.outputPath('config');
	gitbutler = await startGitButler(workdir, configdir, context);

	const fileCPath = gitbutler.pathInWorkdir('local-clone/c_file');

	await gitbutler.runScript('project-with-remote-branches.sh');
	await gitbutler.runScript('apply-upstream-branch.sh', ['branch1', 'local-clone']);

	await page.goto('/');

	// Should load the workspace
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

	// Create a new commit
	writeFileSync(fileCPath, 'This is file C\n', { flag: 'w' });
	await clickByTestId(page, 'start-commit-button');

	// Should see the commit drawer
	await waitForTestId(page, 'new-commit-view');

	const newCommitMessage = 'New local commit: adding file C';
	const newCommitMessageBody = 'CCCCCCC';
	// Write a commit message
	await fillByTestId(page, 'commit-drawer-title-input', newCommitMessage);
	await textEditorFillByTestId(page, 'commit-drawer-description-input', newCommitMessageBody);

	// Click the commit button
	await clickByTestId(page, 'commit-drawer-action-button');

	// Integrate upstream commits
	await clickByTestId(page, 'upstream-commits-integrate-button');
	await waitForTestIdToNotExist(page, 'upstream-commits-integrate-button');
	await waitForTestIdToNotExist(page, 'upstream-commits-commit-action');

	const commitsAfterIntegration = getByTestId(page, 'commit-row');
	await expect(commitsAfterIntegration).toHaveCount(4);
	const firstCommit = commitsAfterIntegration.nth(0);
	await expect(firstCommit).toContainText(newCommitMessage);
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
	await gitbutler.runScript('apply-upstream-branch.sh', ['branch1', 'local-clone']);

	await page.goto('/');

	// Should load the workspace
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
		hasText: 'Conflicting change commit'
	});
	await expect(conflictedCommit).toBeVisible();

	// Click on the conflicted commit to open the commit drawer
	await conflictedCommit.click();

	// Click the resolve conflicts button (now in the file list area)
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
branch1 commit 3
||||||| ancestor
=======
conflicting change
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
branch1 commit 3
conflicting change
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
branch1 commit 3
conflicting change
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
	const header = await waitForTestId(page, 'branch-header');

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
	const header = await waitForTestId(page, 'branch-header');

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

	const commits = getByTestId(page, 'commit-row');
	await expect(commits).toHaveCount(2);

	const conflictedCommit = commits.filter({
		hasText: 'branch1: first commit'
	});
	await expect(conflictedCommit).toBeVisible();

	// Click on the conflicted commit to open the commit drawer
	await conflictedCommit.click();

	// Click the resolve conflicts button (now in the file list area)
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

	// Click on the conflicted commit to open the commit drawer
	await conflictedCommitAfterResolution.click();

	// Click the resolve conflicts button (now in the file list area)
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
	await gitbutler.runScript('apply-upstream-branch.sh', ['branch1', 'local-clone']);

	await page.goto('/');

	// Should load the workspace
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

test('should update the stale selection of an unexisting branch', async ({
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

	// Navigate to branches page
	await clickByTestId(page, 'navigation-branches-button');
	let header = await waitForTestId(page, 'branch-header');

	await expect(header).toContainText('origin/master');

	let branchListCards = getByTestId(page, 'branch-list-card');
	await expect(branchListCards).toHaveCount(3);

	// Select the branch1
	let firstBranchCard = branchListCards.filter({ hasText: 'branch1' });
	await expect(firstBranchCard).toBeVisible();
	await firstBranchCard.click();

	// Go back to the workspace
	await clickByTestId(page, 'navigation-workspace-button');
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
	await waitForTestIdToNotExist(page, 'integrate-upstream-action-button');

	// There should be no stacks
	await waitForTestIdToNotExist(page, 'stack');

	// Navigate to branches page
	await clickByTestId(page, 'navigation-branches-button');
	await waitForTestId(page, 'branches-view');

	header = await waitForTestId(page, 'branch-header');

	await expect(header).toContainText('origin/master');
	// The previously selected branch1 should not be selected anymore
	branchListCards = getByTestId(page, 'branch-list-card');
	await expect(branchListCards).toHaveCount(2);
	firstBranchCard = branchListCards.filter({ hasText: 'branch1' });
	await expect(firstBranchCard).not.toBeVisible();
});

test('should be able to delete a local branch', async ({ page, context }, testInfo) => {
	const workdir = testInfo.outputPath('workdir');
	const configdir = testInfo.outputPath('config');
	gitbutler = await startGitButler(workdir, configdir, context);

	await gitbutler.runScript('project-with-remote-branches.sh');
	await gitbutler.runScript('apply-upstream-branch.sh', ['branch1', 'local-clone']);
	await gitbutler.runScript('apply-upstream-branch.sh', ['branch3', 'local-clone']);
	await gitbutler.runScript('move-branch.sh', ['branch3', 'branch1', 'local-clone']);

	await page.goto('/');

	// Should load the workspace
	await waitForTestId(page, 'workspace-view');

	// There should be one stack with two branches applied
	let stacks = getByTestId(page, 'stack');
	await expect(stacks).toHaveCount(1);
	let branchHeaders = getByTestId(page, 'branch-header');
	await expect(branchHeaders).toHaveCount(2);

	// Right click on the branch header to open the branch menu
	await deleteBranch(page, 'branch1');

	// Should be back in the workspace
	await waitForTestId(page, 'workspace-view');

	// There should be one stack with only one branch
	stacks = getByTestId(page, 'stack');
	await expect(stacks).toHaveCount(1);

	branchHeaders = getByTestId(page, 'branch-header');
	await expect(branchHeaders).toHaveCount(1);
	await expect(branchHeaders.first()).toContainText('branch3');
});

test('should be able to delete an empty local branch', async ({ page, context }, testInfo) => {
	const workdir = testInfo.outputPath('workdir');
	const configdir = testInfo.outputPath('config');
	gitbutler = await startGitButler(workdir, configdir, context);

	await gitbutler.runScript('project-with-remote-branches.sh');

	await page.goto('/');

	// Should load the workspace
	await waitForTestId(page, 'workspace-view');

	// Create a new branch
	await createNewBranch(page, 'new-branch');

	// There should be one stack applied
	let stacks = getByTestId(page, 'stack');
	await expect(stacks).toHaveCount(1);

	// Right click on the branch header to open the branch menu
	await deleteBranch(page, 'new-branch');

	// Should be back in the workspace
	await waitForTestId(page, 'workspace-view');

	// There should be no stacks
	stacks = getByTestId(page, 'stack');
	await expect(stacks).toHaveCount(0);

	const branchHeaders = getByTestId(page, 'branch-header');
	await expect(branchHeaders).toHaveCount(0);
});

test('should be able to unapply a stack', async ({ page, context }, testInfo) => {
	const workdir = testInfo.outputPath('workdir');
	const configdir = testInfo.outputPath('config');
	gitbutler = await startGitButler(workdir, configdir, context);

	await gitbutler.runScript('project-with-remote-branches.sh');
	await gitbutler.runScript('apply-upstream-branch.sh', ['branch1', 'local-clone']);

	await page.goto('/');

	// Should load the workspace
	await waitForTestId(page, 'workspace-view');

	// There should be one stack applied
	let stacks = getByTestId(page, 'stack');
	await expect(stacks).toHaveCount(1);
	let branchHeaders = getByTestId(page, 'branch-header').filter({ hasText: 'branch1' });
	await expect(branchHeaders).toBeVisible();

	// Unapply the stack
	await unapplyStack(page, 'branch1');

	// Should be back in the workspace
	await waitForTestId(page, 'workspace-view');

	// There should be no stacks
	stacks = getByTestId(page, 'stack');
	await expect(stacks).toHaveCount(0);

	branchHeaders = getByTestId(page, 'branch-header').filter({ hasText: 'branch1' });
	await expect(branchHeaders).toHaveCount(0);
});
