import { unstageAllFiles, verifyCommitPlaceholderPosition } from '../src/commit.ts';
import { getHunkLineSelector } from '../src/hunk.ts';
import { getBaseURL, startGitButler, type GitButler } from '../src/setup.ts';
import { clickByTestId, dragAndDropByLocator, getByTestId, waitForTestId } from '../src/util.ts';
import { expect, test } from '@playwright/test';
import { readFileSync, writeFileSync } from 'fs';

let gitbutler: GitButler;

test.use({
	baseURL: getBaseURL()
});

test.setTimeout(300_000);

test.afterEach(async () => {
	await gitbutler?.destroy();
});

function ensureTrailingNewline(content: string): string {
	return content.endsWith('\n') ? content : `${content}\n`;
}

function countLines(content: string): number {
	return ensureTrailingNewline(content).split('\n').length - 1;
}

test('should drag only selected lines when dragging committed hunks', async ({
	page,
	context
}, testInfo) => {
	const workdir = testInfo.outputPath('workdir');
	const configdir = testInfo.outputPath('config');
	gitbutler = await startGitButler(workdir, configdir, context);

	await gitbutler.runScript('project-with-remote-branches.sh');
	await gitbutler.runScript('apply-upstream-branch.sh', ['branch1', 'local-clone']);

	await page.goto('/');
	await waitForTestId(page, 'workspace-view');

	const fileName = 'a_file';
	const filePath = gitbutler.pathInWorkdir(`local-clone/${fileName}`);

	const originalContent = readFileSync(filePath, 'utf-8');
	const originalLineCount = countLines(originalContent);

	const linesToMove = ['MOVE_ME_1', 'MOVE_ME_2'];
	const linesToStay = ['STAY_1', 'STAY_2'];

	const updatedContent =
		ensureTrailingNewline(originalContent) +
		[...linesToMove, ...linesToStay].map((l) => `${l}\n`).join('');
	writeFileSync(filePath, updatedContent, 'utf-8');

	// Create a commit with a multi-line hunk we can partially move.
	await clickByTestId(page, 'start-commit-button');

	const sourceCommitTitle = 'Source: multi-line hunk';
	await waitForTestId(page, 'new-commit-view');
	await verifyCommitPlaceholderPosition(page);
	await unstageAllFiles(page);

	const fileItem = getByTestId(page, 'uncommitted-changes-file-list')
		.getByTestId('file-list-item')
		.filter({ hasText: fileName });
	await expect(fileItem).toBeVisible();

	const fileItemCheckbox = fileItem.locator('input[type="checkbox"]');
	await expect(fileItemCheckbox).toBeVisible();
	await fileItemCheckbox.click();

	await getByTestId(page, 'commit-drawer-title-input').fill(sourceCommitTitle);
	await clickByTestId(page, 'commit-drawer-action-button');

	const sourceCommitRow = getByTestId(page, 'commit-row').filter({ hasText: sourceCommitTitle });
	await expect(sourceCommitRow).toBeVisible();

	// Create a second commit on top so we can move changes "down" the stack without conflicts.
	const otherFileName = 'other-file.txt';
	const otherFilePath = gitbutler.pathInWorkdir(`local-clone/${otherFileName}`);
	writeFileSync(otherFilePath, 'hello\n', 'utf-8');

	await clickByTestId(page, 'start-commit-button');
	await waitForTestId(page, 'new-commit-view');
	await verifyCommitPlaceholderPosition(page);
	await unstageAllFiles(page);

	const otherFileItem = getByTestId(page, 'uncommitted-changes-file-list')
		.getByTestId('file-list-item')
		.filter({ hasText: otherFileName });
	await expect(otherFileItem).toBeVisible();
	await otherFileItem.locator('input[type="checkbox"]').click();

	const destinationCommitTitle = 'Destination: receives selection';
	await getByTestId(page, 'commit-drawer-title-input').fill(destinationCommitTitle);
	await clickByTestId(page, 'commit-drawer-action-button');

	const destinationCommitRow = getByTestId(page, 'commit-row').filter({
		hasText: destinationCommitTitle
	});
	await expect(destinationCommitRow).toBeVisible();

	await sourceCommitRow.click();

	const stackPreview = getByTestId(page, 'stack-preview');
	await expect(stackPreview).toBeVisible();

	const move1LineNumber = originalLineCount + 1;
	const move2LineNumber = originalLineCount + 2;

	// Select only the two MOVE_ME lines within the committed hunk.
	const move1Line = stackPreview
		.locator(getHunkLineSelector(fileName, move1LineNumber, 'right'))
		.first();
	await expect(move1Line).toBeVisible();
	await move1Line.click();

	const move2Line = stackPreview
		.locator(getHunkLineSelector(fileName, move2LineNumber, 'right'))
		.first();
	await expect(move2Line).toBeVisible();
	await move2Line.click();

	const hunkDragHandle = stackPreview
		.locator('.table__title.draggable .table__title-content')
		.first();
	await expect(hunkDragHandle).toBeVisible();

	await dragAndDropByLocator(page, hunkDragHandle, destinationCommitRow);

	// Destination commit should receive only the MOVE_ME lines.
	await destinationCommitRow.click();

	const destinationAddedLines = stackPreview.locator('.table__textContent.diff-line-addition');
	await expect(destinationAddedLines.filter({ hasText: linesToMove[0]! })).toHaveCount(1);
	await expect(destinationAddedLines.filter({ hasText: linesToMove[1]! })).toHaveCount(1);
	await expect(destinationAddedLines.filter({ hasText: linesToStay[0]! })).toHaveCount(0);
	await expect(destinationAddedLines.filter({ hasText: linesToStay[1]! })).toHaveCount(0);

	// Source commit should still add the STAY lines (MOVE_ME becomes context after moving).
	await sourceCommitRow.click();

	const sourceAddedLines = stackPreview.locator('.table__textContent.diff-line-addition');
	await expect(sourceAddedLines.filter({ hasText: linesToStay[0]! })).toHaveCount(1);
	await expect(sourceAddedLines.filter({ hasText: linesToStay[1]! })).toHaveCount(1);
	await expect(sourceAddedLines.filter({ hasText: linesToMove[0]! })).toHaveCount(0);
	await expect(sourceAddedLines.filter({ hasText: linesToMove[1]! })).toHaveCount(0);
});
