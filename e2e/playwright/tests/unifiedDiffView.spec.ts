import { readFileSync, writeFileSync } from 'fs';
import { getBaseURL, startGitButler, type GitButler } from '../src/setup.ts';
import {
	clickByTestId,
	getByTestId,
	sleep,
	waitForTestId,
	waitForTestIdToNotExist
} from '../src/util.ts';
import { expect, test } from '@playwright/test';
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

	const projectPath = gitbutler.pathInWorkdir('local-clone/');
	const bigFilePath = join(projectPath, 'big-file.md');

	await gitbutler.runScript('project-with-remote-branches.sh');

	const contentBefore = readFileSync(BIG_FILE_PATH_BEFORE, 'utf-8');
	const contentAfter = readFileSync(BIG_FILE_PATH_AFTER, 'utf-8');

	// Add the big file on the remote base
	await gitbutler.runScript('project-with-remote-branches__commit-file-into-remote-base.sh', [
		'Create big file with complex changes',
		'big-file.md',
		contentBefore
	]);

	await page.goto('/');

	// Should load the workspace
	await waitForTestId(page, 'workspace-view');

	// Click the sync button
	// await clickByTestId(page, 'sync-button');
	// await sleep(500); // Wait for the sync to start

	// // Integrate upstream commits
	// await clickByTestId(page, 'upstream-commits-integrate-button');
	// await waitForTestIdToNotExist(page, 'upstream-commits-integrate-button');
	// await waitForTestIdToNotExist(page, 'upstream-commits-commit-action');

	// Make the changes to the big file in the local project
	writeFileSync(bigFilePath, contentAfter, 'utf-8');
});
