import { getBaseURL, type GitButler, startGitButler } from '../src/setup.ts';
import { getByTestId, waitForTestId } from '../src/util.ts';
import { expect, test } from '@playwright/test';
import { existsSync } from 'fs';

let gitbutler: GitButler;

test.use({
	baseURL: getBaseURL()
});

test.afterEach(async () => {
	gitbutler?.destroy();
});

test('should detect file creation with touch and deletion with rm', async ({
	page,
	context
}, testInfo) => {
	const workdir = testInfo.outputPath('workdir');
	const configdir = testInfo.outputPath('config');
	gitbutler = await startGitButler(workdir, configdir, context);

	await gitbutler.runScript('setup-empty-project.sh');

	const projectPath = gitbutler.pathInWorkdir('local-clone/');

	await page.goto('/');
	const onboardingPage = getByTestId(page, 'onboarding-page');
	await onboardingPage.waitFor();

	page.click('[data-testid="analytics-continue"]');

	// Add a local project
	const fileChooserPromise = page.waitForEvent('filechooser');
	page.click('[data-testid="add-local-project"]');

	const fileChooser = await fileChooserPromise;
	await fileChooser.setFiles(projectPath);

	// Should see the set target page
	await waitForTestId(page, 'project-setup-page');

	page.click('[data-testid="set-base-branch"]');

	// Should load the workspace directly after setting base branch
	await waitForTestId(page, 'workspace-view');

	// Create a file using touch command
	const testFilePath = gitbutler.pathInWorkdir('local-clone/test-touch-file.txt');
	await gitbutler.runScript('create-test-file.sh');

	// Verify file exists on disk
	expect(existsSync(testFilePath)).toBe(true);

	// Wait for the UI to detect the file
	await waitForTestId(page, 'workspace-view');

	// Should see the uncommitted changes list with the new file
	await waitForTestId(page, 'uncommitted-changes-file-list');
	const filesBeforeDelete = getByTestId(page, 'file-list-item');
	await expect(filesBeforeDelete).toHaveCount(1);
	await expect(filesBeforeDelete.first()).toContainText('test-touch-file.txt');

	// Delete the file using rm command
	await gitbutler.runScript('delete-test-file.sh');

	// Verify file no longer exists on disk
	expect(existsSync(testFilePath)).toBe(false);

	// Wait for the UI to detect the deletion
	await waitForTestId(page, 'workspace-view');

	// The file should no longer be in the uncommitted changes list
	const filesAfterDelete = getByTestId(page, 'file-list-item');
	await expect(filesAfterDelete).toHaveCount(0);
});
