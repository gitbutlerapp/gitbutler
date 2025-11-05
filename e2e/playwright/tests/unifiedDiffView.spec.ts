import { getHunkLineSelector } from '../src/hunk.ts';
import { getBaseURL, startGitButler, type GitButler } from '../src/setup.ts';
import { clickByTestId, fillByTestId, getByTestId, waitForTestId } from '../src/util.ts';
import { expect, Locator, test } from '@playwright/test';
import { readFileSync, writeFileSync } from 'fs';
import { join, resolve } from 'path';

let gitbutler: GitButler;

test.use({
	baseURL: getBaseURL()
});

test.afterEach(async () => {
	await gitbutler?.destroy();
});

const BIG_FILE_PATH_BEFORE = resolve(import.meta.dirname, '../fixtures/big-file_before.md');
const BIG_FILE_PATH_AFTER = resolve(import.meta.dirname, '../fixtures/big-file_after.md');

test('should be able to select the hunks correctly in a complex file', async ({
	page,
	context
}, testInfo) => {
	const workdir = testInfo.outputPath('workdir');
	const configdir = testInfo.outputPath('config');
	gitbutler = await startGitButler(workdir, configdir, context);

	const projectName = 'my-new-project';
	const fileName = 'big-file.md';

	const projectPath = gitbutler.pathInWorkdir(projectName + '/');
	const bigFilePath = join(projectPath, fileName);
	const contentBefore = readFileSync(BIG_FILE_PATH_BEFORE, 'utf-8');
	const contentAfter = readFileSync(BIG_FILE_PATH_AFTER, 'utf-8');

	await gitbutler.runScript('project-with-remote-branches.sh');
	// Add the big file on the remote base
	await gitbutler.runScript('project-with-remote-branches__commit-file-into-remote-base.sh', [
		'Create big file with complex changes',
		fileName,
		contentBefore
	]);
	// Clone into a new project
	await gitbutler.runScript('project-with-remote-branches__clone-into-new-project.sh', [
		projectName
	]);
	// Delete the other project to avoid having to switch between them
	await gitbutler.runScript('project-with-remote-branches__delete-project.sh', ['local-clone']);

	await page.goto('/');

	// Should load the workspace
	await waitForTestId(page, 'workspace-view');

	// Make the changes to the big file in the local project
	writeFileSync(bigFilePath, contentAfter, 'utf-8');

	// Start the commit process
	await clickByTestId(page, 'commit-to-new-branch-button');

	// The file should appear on the uncommitted changes area
	const uncommittedChangesList = getByTestId(page, 'uncommitted-changes-file-list');
	let fileItem = uncommittedChangesList.getByTestId('file-list-item').filter({ hasText: fileName });
	await expect(fileItem).toBeVisible();
	await fileItem.click();

	// The unified diff view should be visible
	const unifiedDiffView = getByTestId(page, 'unified-diff-view');
	await expect(unifiedDiffView).toBeVisible();

	let leftLines = [1, 5, 9, 11, 13, 18, 21];
	let rightLines = [1, 5, 9, 11, 13, 18, 21];

	// Unselect a couple of lines
	await unselectHunkLines(fileName, unifiedDiffView, leftLines, rightLines);

	// Commit the changes
	await fillByTestId(page, 'commit-drawer-title-input', 'Partial commit: Part 1');
	await clickByTestId(page, 'commit-drawer-action-button');

	// Start the commit process
	await clickByTestId(page, 'start-commit-button');

	fileItem = uncommittedChangesList.getByTestId('file-list-item').filter({ hasText: fileName });
	await expect(fileItem).toBeVisible();
	await fileItem.click();

	leftLines = [1, 5, 9, 11];
	rightLines = [1, 5, 9, 11];

	// Unselect a couple of lines
	await unselectHunkLines(fileName, unifiedDiffView, leftLines, rightLines);

	// Commit the changes
	await fillByTestId(page, 'commit-drawer-title-input', 'Partial commit: Part 2');
	await clickByTestId(page, 'commit-drawer-action-button');

	// Start the commit process
	await clickByTestId(page, 'start-commit-button');

	// Commit the changes
	await fillByTestId(page, 'commit-drawer-title-input', 'Full commit: Part 3');
	await clickByTestId(page, 'commit-drawer-action-button');

	// Verify the commits
	const commits = getByTestId(page, 'commit-row');
	await expect(commits).toHaveCount(3);
});

async function unselectHunkLines(
	fileName: string,
	unifiedDiffView: Locator,
	leftLines: number[],
	rightLines: number[]
) {
	for (const line of leftLines) {
		const leftSelector = getHunkLineSelector(fileName, line, 'left');
		const leftLine = unifiedDiffView.locator(leftSelector).first();
		await expect(leftLine).toBeVisible();
		await leftLine.click();
	}

	for (const line of rightLines) {
		const rightSelector = getHunkLineSelector(fileName, line, 'right');
		const rightLine = unifiedDiffView.locator(rightSelector).first();
		await expect(rightLine).toBeVisible();
		await rightLine.click();
	}
}
