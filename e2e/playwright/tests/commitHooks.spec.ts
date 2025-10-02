import { getBaseURL, type GitButler, startGitButler } from '../src/setup.ts';
import { clickByTestId, fillByTestId, getByTestId, waitForTestId } from '../src/util.ts';
import { expect, test } from '@playwright/test';

let gitbutler: GitButler;

test.describe.configure({ mode: 'serial' });

test.use({
	baseURL: getBaseURL()
});

test.afterEach(async () => {
	gitbutler?.destroy();
});

test('should show commit-msg hook rejection error', async ({ page, context }, testInfo) => {
	const workdir = testInfo.outputPath('workdir');
	const configdir = testInfo.outputPath('config');
	gitbutler = await startGitButler(workdir, configdir, context);

	await gitbutler.runScript('project-with-commit-hooks.sh');

	await page.goto('/');

	// Should load the workspace
	await waitForTestId(page, 'workspace-view');

	// Get the project ID from the URL and enable commit hooks
	const url = page.url();
	const projectId = url.split('/')[3]; // URL is /<projectId>/workspace
	await page.evaluate((id) => {
		localStorage.setItem(`projectRunCommitHooks_${id}`, 'true');
	}, projectId);

	// Reload to apply the settings
	await page.reload();
	await waitForTestId(page, 'workspace-view');

	// There should be a file with uncommitted changes
	const fileList = getByTestId(page, 'file-list-item');
	await expect(fileList).toHaveCount(1);
	await expect(fileList).toContainText('uncommitted.txt');

	// Open the commit view
	await clickByTestId(page, 'start-commit-button');

	// The commit view should be visible
	const commitView = getByTestId(page, 'new-commit-view');
	await expect(commitView).toBeVisible();

	// Type a commit message that should be rejected by the hook
	await fillByTestId(page, 'commit-drawer-title-input', 'This message should REJECT');

	// Try to create the commit
	await clickByTestId(page, 'commit-drawer-action-button');

	// Should show an error toast about the hook rejection
	const toastMessage = getByTestId(page, 'toast-info-message');
	await expect(toastMessage).toBeVisible({ timeout: 5000 });
	await expect(toastMessage).toContainText('REJECT');
});

test('should show modified commit message from commit-msg hook', async ({
	page,
	context
}, testInfo) => {
	const workdir = testInfo.outputPath('workdir');
	const configdir = testInfo.outputPath('config');
	gitbutler = await startGitButler(workdir, configdir, context);

	await gitbutler.runScript('project-with-commit-hooks.sh');

	await page.goto('/');

	// Should load the workspace
	await waitForTestId(page, 'workspace-view');

	// Get the project ID from the URL and enable commit hooks
	const url = page.url();
	const projectId = url.split('/')[3]; // URL is /<projectId>/workspace
	await page.evaluate((id) => {
		localStorage.setItem(`projectRunCommitHooks_${id}`, 'true');
	}, projectId);

	// Reload to apply the settings
	await page.reload();
	await waitForTestId(page, 'workspace-view');

	// There should be a file with uncommitted changes
	const fileList = getByTestId(page, 'file-list-item');
	await expect(fileList).toHaveCount(1);

	// Open the commit view
	await clickByTestId(page, 'start-commit-button');

	// The commit view should be visible
	const commitView = getByTestId(page, 'new-commit-view');
	await expect(commitView).toBeVisible();

	// Type a commit message that should be modified by the hook
	await fillByTestId(page, 'commit-drawer-title-input', 'This message should MODIFY');

	// Try to create the commit
	await clickByTestId(page, 'commit-drawer-action-button');

	// The commit should be created successfully
	// Wait for the commit to appear in the commit list
	const commitRow = getByTestId(page, 'commit-row').first();
	await expect(commitRow).toBeVisible({ timeout: 5000 });

	// The commit title should include the [MODIFIED] prefix added by the hook
	await expect(commitRow).toContainText('[MODIFIED]');
});

test('should allow commits when commit-msg hook passes', async ({ page, context }, testInfo) => {
	const workdir = testInfo.outputPath('workdir');
	const configdir = testInfo.outputPath('config');
	gitbutler = await startGitButler(workdir, configdir, context);

	await gitbutler.runScript('project-with-commit-hooks.sh');

	await page.goto('/');

	// Should load the workspace
	await waitForTestId(page, 'workspace-view');

	// Get the project ID from the URL and enable commit hooks
	const url = page.url();
	const projectId = url.split('/')[3]; // URL is /<projectId>/workspace
	await page.evaluate((id) => {
		localStorage.setItem(`projectRunCommitHooks_${id}`, 'true');
	}, projectId);

	// Reload to apply the settings
	await page.reload();
	await waitForTestId(page, 'workspace-view');

	// There should be a file with uncommitted changes
	const fileList = getByTestId(page, 'file-list-item');
	await expect(fileList).toHaveCount(1);

	// Open the commit view
	await clickByTestId(page, 'start-commit-button');

	// Type a normal commit message that should pass
	await fillByTestId(page, 'commit-drawer-title-input', 'A normal commit message');

	// Try to create the commit
	await clickByTestId(page, 'commit-drawer-action-button');

	// The commit should be created successfully
	const commitRow = getByTestId(page, 'commit-row').first();
	await expect(commitRow).toBeVisible({ timeout: 5000 });
	await expect(commitRow).toContainText('A normal commit message');
});

test('should reject commit when pre-commit hook fails', async ({ page, context }, testInfo) => {
	const workdir = testInfo.outputPath('workdir');
	const configdir = testInfo.outputPath('config');
	gitbutler = await startGitButler(workdir, configdir, context);

	await gitbutler.runScript('project-with-commit-hooks.sh');

	await page.goto('/');

	// Should load the workspace
	await waitForTestId(page, 'workspace-view');

	// Get the project ID from the URL and enable commit hooks
	const url = page.url();
	const projectId = url.split('/')[3];
	await page.evaluate((id) => {
		localStorage.setItem(`projectRunCommitHooks_${id}`, 'true');
	}, projectId);

	// Reload to apply the settings
	await page.reload();
	await waitForTestId(page, 'workspace-view');

	// Create a file with forbidden content
	await gitbutler.runScript('create-forbidden-file.sh');

	// Wait for the file to appear in the UI
	await page.waitForTimeout(500);
	await page.reload();
	await waitForTestId(page, 'workspace-view');

	// There should be files with uncommitted changes
	const fileList = getByTestId(page, 'file-list-item');
	await expect(fileList).toHaveCount(2); // uncommitted.txt and forbidden.txt

	// Open the commit view
	await clickByTestId(page, 'start-commit-button');

	// Type a commit message
	await fillByTestId(page, 'commit-drawer-title-input', 'Adding forbidden file');

	// Try to create the commit
	await clickByTestId(page, 'commit-drawer-action-button');

	// Should show an error toast about the pre-commit hook rejection
	const toastMessage = getByTestId(page, 'toast-info-message');
	await expect(toastMessage).toBeVisible({ timeout: 5000 });
	await expect(toastMessage).toContainText('FORBIDDEN');
});

test('should allow commit when pre-commit hook passes', async ({ page, context }, testInfo) => {
	const workdir = testInfo.outputPath('workdir');
	const configdir = testInfo.outputPath('config');
	gitbutler = await startGitButler(workdir, configdir, context);

	await gitbutler.runScript('project-with-commit-hooks.sh');

	await page.goto('/');

	// Should load the workspace
	await waitForTestId(page, 'workspace-view');

	// Get the project ID from the URL and enable commit hooks
	const url = page.url();
	const projectId = url.split('/')[3];
	await page.evaluate((id) => {
		localStorage.setItem(`projectRunCommitHooks_${id}`, 'true');
	}, projectId);

	// Reload to apply the settings
	await page.reload();
	await waitForTestId(page, 'workspace-view');

	// Create a file with allowed content
	await gitbutler.runScript('create-allowed-file.sh');

	// Wait for the file to appear in the UI
	await page.waitForTimeout(500);
	await page.reload();
	await waitForTestId(page, 'workspace-view');

	// Open the commit view
	await clickByTestId(page, 'start-commit-button');

	// Type a commit message
	await fillByTestId(page, 'commit-drawer-title-input', 'Adding allowed file');

	// Try to create the commit
	await clickByTestId(page, 'commit-drawer-action-button');

	// The commit should be created successfully (pre-commit hook passed)
	const commitRow = getByTestId(page, 'commit-row').first();
	await expect(commitRow).toBeVisible({ timeout: 10000 });
	await expect(commitRow).toContainText('Adding allowed file');
});

test('should show post-commit hook success', async ({ page, context }, testInfo) => {
	const workdir = testInfo.outputPath('workdir');
	const configdir = testInfo.outputPath('config');
	gitbutler = await startGitButler(workdir, configdir, context);

	await gitbutler.runScript('project-with-commit-hooks.sh');

	await page.goto('/');

	// Should load the workspace
	await waitForTestId(page, 'workspace-view');

	// Get the project ID from the URL and enable commit hooks
	const url = page.url();
	const projectId = url.split('/')[3];
	await page.evaluate((id) => {
		localStorage.setItem(`projectRunCommitHooks_${id}`, 'true');
	}, projectId);

	// Reload to apply the settings
	await page.reload();
	await waitForTestId(page, 'workspace-view');

	// There should be a file with uncommitted changes
	const fileList = getByTestId(page, 'file-list-item');
	await expect(fileList.first()).toBeVisible();

	// Open the commit view
	await clickByTestId(page, 'start-commit-button');

	// Type a commit message
	await fillByTestId(page, 'commit-drawer-title-input', 'Testing post-commit hook');

	// Try to create the commit
	await clickByTestId(page, 'commit-drawer-action-button');

	// The commit should be created successfully
	const commitRow = getByTestId(page, 'commit-row').first();
	await expect(commitRow).toBeVisible({ timeout: 10000 });
	await expect(commitRow).toContainText('Testing post-commit hook');

	// Wait for post-commit hook to execute (it runs in background)
	await page.waitForTimeout(1500);
});

test('should show post-commit hook failure but commit still created', async ({
	page,
	context
}, testInfo) => {
	const workdir = testInfo.outputPath('workdir');
	const configdir = testInfo.outputPath('config');
	gitbutler = await startGitButler(workdir, configdir, context);

	await gitbutler.runScript('project-with-commit-hooks.sh');

	await page.goto('/');

	// Should load the workspace
	await waitForTestId(page, 'workspace-view');

	// Get the project ID from the URL and enable commit hooks
	const url = page.url();
	const projectId = url.split('/')[3];
	await page.evaluate((id) => {
		localStorage.setItem(`projectRunCommitHooks_${id}`, 'true');
	}, projectId);

	// Reload to apply the settings
	await page.reload();
	await waitForTestId(page, 'workspace-view');

	// Create a marker file that will trigger post-commit failure
	await gitbutler.runScript('create-postcommit-fail-marker.sh');

	// Wait for the file to appear in the UI
	await page.waitForTimeout(500);
	await page.reload();
	await waitForTestId(page, 'workspace-view');

	// Open the commit view
	await clickByTestId(page, 'start-commit-button');

	// Type a commit message
	await fillByTestId(page, 'commit-drawer-title-input', 'Trigger post-commit failure');

	// Try to create the commit
	await clickByTestId(page, 'commit-drawer-action-button');

	// The commit should be created (pre-commit passed)
	const commitRow = getByTestId(page, 'commit-row').first();
	await expect(commitRow).toBeVisible({ timeout: 10000 });
	await expect(commitRow).toContainText('Trigger post-commit failure');

	// Wait for post-commit hook to run and fail (it runs in background)
	await page.waitForTimeout(1500);
});
