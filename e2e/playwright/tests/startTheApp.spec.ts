import { getBaseURL, type GitButler, startGitButler } from '../src/setup.ts';
import { clickByTestId, getByTestId } from '../src/util.ts';
import { test } from '@playwright/test';

let gitbutler: GitButler;

test.use({
	baseURL: getBaseURL()
});

test.afterEach(async () => {
	gitbutler?.destroy();
});

test('should start the application', async ({ page, context }, testInfo) => {
	const workdir = testInfo.outputPath('workdir');
	const configdir = testInfo.outputPath('config');
	gitbutler = await startGitButler(workdir, configdir, context);

	await gitbutler.runScript('setup-empty-project.sh');

	await page.goto('/');
	const onboardingPage = getByTestId(page, 'onboarding-page');
	await onboardingPage.waitFor();

	clickByTestId(page, 'analytics-continue');

	// Add a local project
	const fileChooserPromise = page.waitForEvent('filechooser');
	clickByTestId(page, 'add-local-project');

	const fileChooser = await fileChooserPromise;
	const projectPath = gitbutler.pathInWorkdir('local-clone/');
	await fileChooser.setFiles(projectPath);

	// Should see the set target page
	const projectSetupPage = getByTestId(page, 'project-setup-page');
	await projectSetupPage.waitFor();

	clickByTestId(page, 'set-base-branch');

	// Should see the keys form page
	const gitAuthPage = getByTestId(page, 'project-setup-git-auth-page');
	await gitAuthPage.waitFor();
	clickByTestId(page, 'accept-git-auth');

	// Should load the workspace
	const workspaceView = getByTestId(page, 'workspace-view');
	await workspaceView.waitFor();
});
