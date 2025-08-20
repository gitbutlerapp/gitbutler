import { type GitButler, startGitButler } from '../src/setup.ts';
import { getByTestId } from '../src/util.ts';
import { test } from '@playwright/test';

let gitbutler: GitButler;

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

	const continueButton = getByTestId(page, 'analytics-continue');
	await continueButton.waitFor();
	await continueButton.click();

	// Add a local project
	const fileChooserPromise = page.waitForEvent('filechooser');
	const addLocalProjectButton = getByTestId(page, 'add-local-project');
	await addLocalProjectButton.waitFor();
	await addLocalProjectButton.click();

	const fileChooser = await fileChooserPromise;
	const projectPath = gitbutler.pathInWorkdir('local-clone/');
	await fileChooser.setFiles(projectPath);

	// Should see the set target page
	const projectSetupPage = getByTestId(page, 'project-setup-page');
	await projectSetupPage.waitFor();

	const targetBranchSelect = getByTestId(page, 'set-base-branch');
	await targetBranchSelect.waitFor();
	await targetBranchSelect.click();

	// Should see the keys form page
	const gitAuthPage = getByTestId(page, 'project-setup-git-auth-page');
	await gitAuthPage.waitFor();
	const acceptGitAuthButton = getByTestId(page, 'accept-git-auth');
	await acceptGitAuthButton.waitFor();
	await acceptGitAuthButton.click();

	// Should load the workspace
	const workspaceView = getByTestId(page, 'workspace-view');
	await workspaceView.waitFor();
});
