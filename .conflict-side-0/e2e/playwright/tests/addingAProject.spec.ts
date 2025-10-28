import { getBaseURL, startGitButler, type GitButler } from '../src/setup.ts';
import { clickByTestId, getByTestId, waitForTestId } from '../src/util.ts';
import { expect, test } from '@playwright/test';

let gitbutler: GitButler;

test.use({
	baseURL: getBaseURL()
});

test.afterEach(async () => {
	await gitbutler?.destroy();
});

test('should handle gracefully adding an existing project', async ({ page, context }, testInfo) => {
	const workdir = testInfo.outputPath('workdir');
	const configdir = testInfo.outputPath('config');
	gitbutler = await startGitButler(workdir, configdir, context);

	const projectPath = gitbutler.pathInWorkdir('local-clone-2/');

	await gitbutler.runScript('two-projects-with-remote-branches.sh');

	await page.goto('/');

	// Should load the workspace
	await waitForTestId(page, 'workspace-view');

	// Open the project selector
	await clickByTestId(page, 'chrome-header-project-selector');
	// Click the add local project button
	const fileChooserPromise = page.waitForEvent('filechooser');
	await clickByTestId(page, 'chrome-header-project-selector-add-local-project');

	const fileChooser = await fileChooserPromise;
	await fileChooser.setFiles(projectPath);

	// Should display the "project already exists" modal
	await waitForTestId(page, 'add-project-already-exists-modal');
	// Click it in order to close the select dropdown behind
	await clickByTestId(page, 'add-project-already-exists-modal', true);

	// Click to open the existing project
	await clickByTestId(page, 'add-project-already-exists-modal-open-project-button');

	// Should navigate to the existing project
	const projectSelector = getByTestId(page, 'chrome-header-project-selector');
	await expect(projectSelector).toContainText('local-clone-2');
});

test('should handle gracefully adding bare repo', async ({ page, context }, testInfo) => {
	const workdir = testInfo.outputPath('workdir');
	const configdir = testInfo.outputPath('config');
	gitbutler = await startGitButler(workdir, configdir, context);

	const projectPath = gitbutler.pathInWorkdir('local-clone/');

	await gitbutler.runScript('setup-empty-project-bare.sh');

	await page.goto('/');
	const onboardingPage = getByTestId(page, 'onboarding-page');
	await onboardingPage.waitFor();

	clickByTestId(page, 'analytics-continue');

	// Add a local project
	const fileChooserPromise = page.waitForEvent('filechooser');
	clickByTestId(page, 'add-local-project');

	const fileChooser = await fileChooserPromise;
	await fileChooser.setFiles(projectPath);

	await waitForTestId(page, 'add-project-bare-repo-modal');
});

test('should handle gracefully adding a non-git directory', async ({ page, context }, testInfo) => {
	const workdir = testInfo.outputPath('workdir');
	const configdir = testInfo.outputPath('config');
	gitbutler = await startGitButler(workdir, configdir, context);

	const projectPath = gitbutler.pathInWorkdir('not-a-git-repo/');

	await gitbutler.runScript('setup-empty-project.sh');

	await page.goto('/');
	const onboardingPage = getByTestId(page, 'onboarding-page');
	await onboardingPage.waitFor();

	clickByTestId(page, 'analytics-continue');

	// Add a local project
	const fileChooserPromise = page.waitForEvent('filechooser');
	clickByTestId(page, 'add-local-project');

	const fileChooser = await fileChooserPromise;
	await fileChooser.setFiles(projectPath);

	await waitForTestId(page, 'add-project-not-a-git-repo-modal');
});

test('should handle adding a project with extra commit and uncommitted changes on main branch', async ({
	page,
	context
}, testInfo) => {
	const workdir = testInfo.outputPath('workdir');
	const configdir = testInfo.outputPath('config');
	gitbutler = await startGitButler(workdir, configdir, context);

	const projectPath = gitbutler.pathInWorkdir('local-with-changes/');

	// Set up repository with extra commit and uncommitted changes
	await gitbutler.runScript('project-with-commit-and-uncommitted-changes.sh');

	await page.goto('/');

	const onboardingPage = getByTestId(page, 'onboarding-page');
	await onboardingPage.waitFor();

	clickByTestId(page, 'analytics-continue');

	// Click the add local project button
	const fileChooserPromise = page.waitForEvent('filechooser');
	clickByTestId(page, 'add-local-project');

	const fileChooser = await fileChooserPromise;
	await fileChooser.setFiles(projectPath);

	clickByTestId(page, 'set-base-branch');

	// Should load the workspace directly after setting base branch
	await waitForTestId(page, 'workspace-view');

	// There should be only one stack
	const stacks = getByTestId(page, 'stack');
	await expect(stacks).toHaveCount(1);
	const stack = stacks.first();

	// The stack should not have the branch called "master"
	await expect(stack).not.toContainText('master');
});
