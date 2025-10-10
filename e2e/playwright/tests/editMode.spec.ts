import { getBaseURL, startGitButler, type GitButler } from '../src/setup.ts';
import { clickByTestId, waitForTestId } from '../src/util.ts';
import { expect, test } from '@playwright/test';
import { readFileSync, writeFileSync } from 'fs';

let gitbutler: GitButler;

test.use({
	baseURL: getBaseURL()
});

test.afterEach(async () => {
	gitbutler?.destroy();
});

test('should be able to edit a commit through the edit mode', async ({
	page,
	context
}, testInfo) => {
	const workdir = testInfo.outputPath('workdir');
	const configdir = testInfo.outputPath('config');
	gitbutler = await startGitButler(workdir, configdir, context);

	const fileBPath = gitbutler.pathInWorkdir('file-b.txt');

	await gitbutler.runScript('project-with-remote-branches.sh');
	await gitbutler.runScript('apply-upstream-branch.sh', ['branch1', 'local-clone']);

	await page.goto('/');

	// Should load the workspace
	await waitForTestId(page, 'workspace-view');

	const commitRows = page.getByTestId('commit-row');
	await expect(commitRows).toHaveCount(2);
	const bottomCommit = commitRows.filter({ hasText: 'branch1: first commit' });

	bottomCommit.click({ button: 'right' });
	await clickByTestId(page, 'commit-row-context-menu-edit-commit');
	// Should open the edit mode
	await waitForTestId(page, 'edit-mode');

	// Create another file to commit
	writeFileSync(fileBPath, 'This is file B\n', { encoding: 'utf-8' });

	// Click the save and exit button
	await clickByTestId(page, 'edit-mode-save-and-exit-button');

	// Should be back in the workspace
	await waitForTestId(page, 'workspace-view');

	const fileBContent = readFileSync(fileBPath, { encoding: 'utf-8' });

	expect(fileBContent).toEqual('This is file B\n');
});
