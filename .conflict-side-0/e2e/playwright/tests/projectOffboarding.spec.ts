import { getBaseURL, type GitButler, setCookie, startGitButler } from '../src/setup.ts';
import { clickByTestId, waitForTestId, waitForTestIdToNotExist } from '../src/util.ts';
import { test } from '@playwright/test';

let gitbutler: GitButler;

test.use({
	baseURL: getBaseURL()
});

test.afterEach(async () => {
	await gitbutler?.destroy();
});

test('should be able to delete the last project gracefuly', async ({ page, context }, testInfo) => {
	const workdir = testInfo.outputPath('workdir');
	const configdir = testInfo.outputPath('config');
	gitbutler = await startGitButler(workdir, configdir, context);

	// Override the analytics confirmation so we don't see the page later
	await setCookie('disk-store-override:appAnalyticsConfirmed', 'true', context);

	await gitbutler.runScript('project-with-remote-branches.sh');

	await page.goto('/');

	// Should load the workspace
	await waitForTestId(page, 'workspace-view');

	// Open project settings
	await clickByTestId(page, 'chrome-sidebar-project-settings-button');

	await waitForTestId(page, 'project-settings-modal');

	const deleteProjectButton = await waitForTestId(page, 'project-delete-button');
	await deleteProjectButton.scrollIntoViewIfNeeded();
	await deleteProjectButton.click();

	await clickByTestId(page, 'project-delete-modal-confirm');

	await waitForTestIdToNotExist(page, 'project-delete-modal-confirm');
	await waitForTestIdToNotExist(page, 'project-delete-button');
	await waitForTestIdToNotExist(page, 'project-settings-modal');

	await waitForTestId(page, 'welcome-page');
});
