import {
	openCommitDrawer,
	stageFirstFile,
	startEditingCommitMessage,
	unstageAllFiles,
	updateCommitMessage,
	verifyCommitDrawerContent,
	verifyCommitMessageEditor,
	verifyCommitPlaceholderPosition
} from '../src/commit.ts';
import { writeFiles } from '../src/file.ts';
import { getBaseURL, type GitButler, startGitButler } from '../src/setup.ts';
import {
	clickByTestId,
	dragAndDropByLocator,
	getByTestId,
	waitForTestId,
	waitForTestIdToNotExist
} from '../src/util.ts';
import { expect, Page, test } from '@playwright/test';
import { copyFileSync, writeFileSync } from 'fs';
import { join } from 'path';

let gitbutler: GitButler;

test.use({
	baseURL: getBaseURL()
});

test.afterEach(async () => {
	await gitbutler.destroy();
});

const FIXTURE_IMAGE_PATH = join(import.meta.dirname, '../fixtures/lesh0.jpg');

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

test('should be able to commit a bunch of times in a row and edit their message', async ({
	page,
	context
}, testInfo) => {
	const workdir = testInfo.outputPath('workdir');
	const configdir = testInfo.outputPath('config');
	gitbutler = await startGitButler(workdir, configdir, context);

	await gitbutler.runScript('project-with-remote-branches.sh');
	await gitbutler.runScript('apply-upstream-branch.sh', ['branch1', 'local-clone']);

	// Create a couple of uncommitted files.
	const fileNames = ['file1.txt', 'file2.txt', 'file3.txt', 'file4.txt', 'file5.txt', 'file6.txt'];
	const filesContent: Record<string, string> = {};
	for (const fileName of fileNames) {
		const filePath = gitbutler.pathInWorkdir(`local-clone/${fileName}`);
		filesContent[filePath] = `This is ${fileName}\n`;
	}
	writeFiles(filesContent);

	await page.goto('/');

	// Should load the workspace
	await waitForTestId(page, 'workspace-view');

	const TIMES = 3;
	await commitMultipleTimes(TIMES, page);
	await amendCommitMessageMultipleTimes(TIMES - 1, page);
	await startAmendingACommitMessageAndCancel(page, TIMES - 1);
	await startCommittingAndCancel(page, TIMES);
});

test('should be able to commit a binary file', async ({ page, context }, testInfo) => {
	const workdir = testInfo.outputPath('workdir');
	const configdir = testInfo.outputPath('config');
	gitbutler = await startGitButler(workdir, configdir, context);

	await gitbutler.runScript('project-with-remote-branches.sh');
	await gitbutler.runScript('apply-upstream-branch.sh', ['branch1', 'local-clone']);

	// Copy the binary image file from fixtures to the working directory
	const targetImagePath = gitbutler.pathInWorkdir('local-clone/lesh0.jpg');
	copyFileSync(FIXTURE_IMAGE_PATH, targetImagePath);

	await page.goto('/');

	// Should load the workspace
	await waitForTestId(page, 'workspace-view');

	// There should be only one stack
	const stacks = getByTestId(page, 'stack');
	await expect(stacks).toHaveCount(1);

	// The binary file should appear in the uncommitted changes
	const fileLocator = getByTestId(page, 'file-list-item').filter({ hasText: 'lesh0.jpg' });
	await expect(fileLocator).toBeVisible();

	// Start committing the binary file
	await clickByTestId(page, 'start-commit-button');

	await verifyCommitPlaceholderPosition(page);
	await unstageAllFiles(page);

	// Stage the binary file
	const imageFileCheckbox = fileLocator.locator('input[type="checkbox"]');
	await expect(imageFileCheckbox).toBeVisible();
	await imageFileCheckbox.click();

	const commitTitle = 'Add binary image file';
	const commitMessage = 'Adding lesh0.jpg to the repository';

	// Fill the commit message
	await verifyCommitMessageEditor(page, '', '');
	await updateCommitMessage(page, commitTitle, commitMessage);

	// Submit the commit
	await clickByTestId(page, 'commit-drawer-action-button');

	// Commit with title should be visible in the commit list
	const commitRow = getByTestId(page, 'commit-row').filter({ hasText: commitTitle });
	await expect(commitRow).toBeVisible();

	// Open the commit drawer to verify the binary file was committed
	const commitDrawer = await openCommitDrawer(page, commitTitle);
	await verifyCommitDrawerContent(commitDrawer, commitTitle, commitMessage);

	const stackPreview = getByTestId(page, 'stack');
	// Verify the binary file is listed in the commit
	const committedFile = stackPreview.getByTestId('file-list-item').filter({ hasText: 'lesh0.jpg' });
	await expect(committedFile).toBeVisible();
});

test('should be able to commit a git submodule', async ({ page, context }, testInfo) => {
	const workdir = testInfo.outputPath('workdir');
	const configdir = testInfo.outputPath('config');
	gitbutler = await startGitButler(workdir, configdir, context);

	await gitbutler.runScript('project-with-remote-branches.sh');
	await gitbutler.runScript('apply-upstream-branch.sh', ['branch1', 'local-clone']);

	await page.goto('/');

	// Should load the workspace
	await waitForTestId(page, 'workspace-view');

	// There should be only one stack
	const stacks = getByTestId(page, 'stack');
	await expect(stacks).toHaveCount(1);

	// Add a git submodule to the working directory
	await gitbutler.runScript('project-with-remote-branches__add-submodule.sh');

	// The submodule files should appear in the uncommitted changes
	const gitmodulesFile = getByTestId(page, 'file-list-item').filter({ hasText: '.gitmodules' });
	const submoduleDir = getByTestId(page, 'file-list-item').filter({ hasText: 'my-submodule' });
	await expect(gitmodulesFile).toBeVisible();
	await expect(submoduleDir).toBeVisible();

	// Start committing the submodule
	await clickByTestId(page, 'start-commit-button');

	await verifyCommitPlaceholderPosition(page);
	await unstageAllFiles(page);

	// Stage the .gitmodules file and submodule directory
	const gitmodulesCheckbox = gitmodulesFile.locator('input[type="checkbox"]');
	await expect(gitmodulesCheckbox).toBeVisible();
	await gitmodulesCheckbox.click();

	const submoduleCheckbox = submoduleDir.locator('input[type="checkbox"]');
	await expect(submoduleCheckbox).toBeVisible();
	await submoduleCheckbox.click();

	const commitTitle = 'Add git submodule';
	const commitMessage = 'Adding my-submodule to the repository';

	// Fill the commit message
	await verifyCommitMessageEditor(page, '', '');
	await updateCommitMessage(page, commitTitle, commitMessage);

	// Submit the commit
	await clickByTestId(page, 'commit-drawer-action-button');

	// Commit with title should be visible in the commit list
	const commitRow = getByTestId(page, 'commit-row').filter({ hasText: commitTitle });
	await expect(commitRow).toBeVisible();

	// Open the commit drawer to verify the submodule was committed
	const commitDrawer = await openCommitDrawer(page, commitTitle);
	await verifyCommitDrawerContent(commitDrawer, commitTitle, commitMessage);

	const stackPreview = getByTestId(page, 'stack');
	// Verify the .gitmodules file is listed in the commit
	const committedGitmodules = stackPreview
		.getByTestId('file-list-item')
		.filter({ hasText: '.gitmodules' });
	await expect(committedGitmodules).toBeVisible();

	// Verify the submodule directory is listed in the commit
	const committedSubmodule = stackPreview
		.getByTestId('file-list-item')
		.filter({ hasText: 'my-submodule' });
	await expect(committedSubmodule).toBeVisible();
});

/**
 * Commit multiple times in a row.
 *
 * Each time, only the first file will be staged and committed.
 */
async function commitMultipleTimes(TIMES: number, page: Page) {
	for (let i = 0; i < TIMES; i++) {
		// Start committing the files one by one
		await clickByTestId(page, 'start-commit-button');

		await verifyCommitPlaceholderPosition(page);
		await unstageAllFiles(page);
		await stageFirstFile(page);

		const commitTitle = getCommitTitle(i);
		const commitMessage = getCommitDescription(i);

		// Fill the commit message
		await verifyCommitMessageEditor(page, '', '');
		await updateCommitMessage(page, commitTitle, commitMessage);

		// Submit the commit
		await clickByTestId(page, 'commit-drawer-action-button');

		// Commit with title should be visible in the commit list
		const commitRow = getByTestId(page, 'commit-row').filter({ hasText: commitTitle });
		await expect(commitRow).toBeVisible();
	}
}

async function startCommittingAndCancel(page: Page, index: number) {
	// Start committing the files one by one
	await clickByTestId(page, 'start-commit-button');

	await verifyCommitPlaceholderPosition(page);
	await unstageAllFiles(page);
	await stageFirstFile(page);

	const commitTitle = getCommitTitle(index);
	const commitMessage = getCommitDescription(index);

	// Fill the commit message
	await verifyCommitMessageEditor(page, '', '');
	await updateCommitMessage(page, commitTitle, commitMessage);

	// Cancel the commit
	await clickByTestId(page, 'commit-drawer-cancel-button');

	// Commit with title should be visible in the commit list
	const commitRow = getByTestId(page, 'commit-row').filter({ hasText: commitTitle });
	await expect(commitRow).toHaveCount(0);
}

/**
 * Amend the commit message of multiple commits in a row.
 */
async function amendCommitMessageMultipleTimes(TIMES: number, page: Page) {
	for (let i = 0; i < TIMES; i++) {
		const commitTitle = getCommitTitle(i);
		const commitMessage = getCommitDescription(i);
		const newCommitTitle = getAmendedCommitTitle(i);
		const newCommitMessage = getAmendedCommitDescription(i);

		// Open the commit drawer and verify initial content
		const commitDrawer = await openCommitDrawer(page, commitTitle);
		await verifyCommitDrawerContent(commitDrawer, commitTitle, commitMessage);

		// Start editing the commit message
		await startEditingCommitMessage(page, commitDrawer);
		await verifyCommitMessageEditor(page, commitTitle, commitMessage);

		// Update and submit the changes
		await updateCommitMessage(page, newCommitTitle, newCommitMessage);
		await clickByTestId(page, 'commit-drawer-action-button');

		// Verify the changes were applied
		await waitForTestIdToNotExist(page, 'edit-commit-message-box');
		await verifyCommitDrawerContent(commitDrawer, newCommitTitle, newCommitMessage);
	}
}

async function startAmendingACommitMessageAndCancel(page: Page, commitIndex: number) {
	const commitTitle = getCommitTitle(commitIndex);
	const commitMessage = getCommitDescription(commitIndex);
	const newCommitTitle = getAmendedCommitTitle(commitIndex);
	const newCommitMessage = getAmendedCommitDescription(commitIndex);

	// Open the commit drawer and verify initial content
	const commitDrawer = await openCommitDrawer(page, commitTitle);
	await verifyCommitDrawerContent(commitDrawer, commitTitle, commitMessage);

	// Start editing the commit message
	await startEditingCommitMessage(page, commitDrawer);
	await verifyCommitMessageEditor(page, commitTitle, commitMessage);

	// Update and cancel the changes
	await updateCommitMessage(page, newCommitTitle, newCommitMessage);
	await clickByTestId(page, 'commit-drawer-cancel-button');

	// Verify the changes were NOT applied (original values remain)
	await waitForTestIdToNotExist(page, 'edit-commit-message-box');
	await verifyCommitDrawerContent(commitDrawer, commitTitle, commitMessage);
}

function getCommitTitle(i: number): string {
	return `Commit number ${i + 1}`;
}

function getAmendedCommitTitle(i: number): string {
	return `Amended Commit number ${i + 1}`;
}

function getCommitDescription(i: number): string {
	return `Description for commit ${i + 1}`;
}

function getAmendedCommitDescription(i: number): string {
	return `Amended description for commit ${i + 1}`;
}
