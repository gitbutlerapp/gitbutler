import { getBaseURL, type GitButler, startGitButler } from '../src/setup.ts';
import { clickByTestId, getByTestId, waitForTestId } from '../src/util.ts';
import { expect, test } from '@playwright/test';
import path from 'node:path';

/**
 * Claude permission flow E2E tests.
 *
 * These tests require the application to be built with the `claude-testing` feature:
 *   cargo build -p gitbutler-tauri --features claude-testing
 *
 * Each test specifies its own mock scenario file via CLAUDE_MOCK_SCENARIO env var
 * passed to the but-server process at startup.
 */

let gitbutler: GitButler;

// Path to the mock scenario files
const SCENARIO_DIR = path.resolve(import.meta.dirname, '../fixtures/claude-scenarios');

test.use({
	baseURL: getBaseURL(),
	// These tests spawn their own but-server process which may need to compile,
	// so we need a longer timeout than the default 30 seconds
	actionTimeout: 60_000
});

// Set a longer test timeout to account for but-server startup/compilation
test.setTimeout(120_000);

test.afterEach(async () => {
	await gitbutler?.destroy();
});

test.describe('Claude permission approval flow', () => {
	test('should show permission popup when Claude requests Bash tool use', async ({
		page,
		context
	}, testInfo) => {
		const workdir = testInfo.outputPath('workdir');
		const configdir = testInfo.outputPath('config');

		// Start GitButler with the mock scenario
		gitbutler = await startGitButler(workdir, configdir, context, {
			CLAUDE_MOCK_SCENARIO: path.join(SCENARIO_DIR, 'permission-bash-echo.json'),
			RUST_LOG: 'but_claude=debug,claude_agent_sdk_rs=debug,error'
		});

		// Set up a project with a stack to work with
		await gitbutler.runScript('project-with-remote-branches.sh');

		await page.goto('/');

		// Should load the workspace directly (project already set up by script)
		await waitForTestId(page, 'workspace-view');

		// Click the AI button on the existing stack to open codegen view
		await clickByTestId(page, 'branch-header-context-menu-start-codegen-agent');

		// Wait for input to appear
		await waitForTestId(page, 'codegen-input');

		// Type a message to Claude
		const input = getByTestId(page, 'codegen-input');
		await input.locator('div[contenteditable="true"]').fill('Please run echo hello world');

		// Send the message
		await clickByTestId(page, 'codegen-input-send-button');

		// Wait for the permission approval popup to appear (with timeout for backend processing)
		const permissionPopup = await waitForTestId(page, 'codegen-permission-approval');
		await expect(permissionPopup).toBeVisible();

		// Verify the popup shows the Bash tool
		await expect(permissionPopup).toContainText('Bash');
		await expect(permissionPopup).toContainText('echo hello world');

		// Click the Allow button
		await clickByTestId(page, 'codegen-permission-approval-allow-button');

		// Verify the popup disappears
		await expect(permissionPopup).not.toBeVisible();
	});

	test('should allow changing permission scope before approval', async ({
		page,
		context
	}, testInfo) => {
		const workdir = testInfo.outputPath('workdir');
		const configdir = testInfo.outputPath('config');

		gitbutler = await startGitButler(workdir, configdir, context, {
			CLAUDE_MOCK_SCENARIO: path.join(SCENARIO_DIR, 'permission-wildcard-test.json')
		});

		// Set up a project with a stack to work with
		await gitbutler.runScript('project-with-remote-branches.sh');

		await page.goto('/');
		await waitForTestId(page, 'workspace-view');

		// Click the AI button on the existing stack to open codegen view
		await clickByTestId(page, 'branch-header-context-menu-start-codegen-agent');
		await waitForTestId(page, 'codegen-input');

		const input = getByTestId(page, 'codegen-input');
		await input.locator('div[contenteditable="true"]').fill('Please run echo hello world');
		await clickByTestId(page, 'codegen-input-send-button');

		const permissionPopup = await waitForTestId(page, 'codegen-permission-approval');
		await expect(permissionPopup).toBeVisible();

		// Click the wildcard button to change scope
		const wildcardButton = getByTestId(page, 'codegen-permission-approval-wildcard-button');
		await expect(wildcardButton).toBeVisible();

		// Initially should show "This command"
		await expect(wildcardButton).toContainText('This command');

		// Click to open the dropdown
		await wildcardButton.click();

		// Select "Any subcommands or flags" option
		await page.getByText('Any subcommands or flags').click();

		// Verify the button now shows the new selection
		await expect(wildcardButton).toContainText('Any subcommands or flags');

		// Now approve - the mock scenario sends a second command after this
		await clickByTestId(page, 'codegen-permission-approval-allow-button');

		// Wait for the second permission request to appear (echo world)
		// This tests that the permission flow continues for additional tool uses
		await expect(permissionPopup.getByText('echo world')).toBeVisible({ timeout: 5000 });

		// Approve the second command as well
		await clickByTestId(page, 'codegen-permission-approval-allow-button');

		// Now the popup should finally disappear
		await expect(permissionPopup).not.toBeVisible();
	});

	test('should allow denying permission', async ({ page, context }, testInfo) => {
		const workdir = testInfo.outputPath('workdir');
		const configdir = testInfo.outputPath('config');

		gitbutler = await startGitButler(workdir, configdir, context, {
			CLAUDE_MOCK_SCENARIO: path.join(SCENARIO_DIR, 'permission-bash-echo.json')
		});

		// Set up a project with a stack to work with
		await gitbutler.runScript('project-with-remote-branches.sh');

		await page.goto('/');
		await waitForTestId(page, 'workspace-view');

		// Click the AI button on the existing stack to open codegen view
		await clickByTestId(page, 'branch-header-context-menu-start-codegen-agent');
		await waitForTestId(page, 'codegen-input');

		const input = getByTestId(page, 'codegen-input');
		await input.locator('div[contenteditable="true"]').fill('Please run echo hello world');
		await clickByTestId(page, 'codegen-input-send-button');

		const permissionPopup = await waitForTestId(page, 'codegen-permission-approval');
		await expect(permissionPopup).toBeVisible();

		// Click the Deny button
		await clickByTestId(page, 'codegen-permission-approval-deny-button');

		// Verify the popup disappears
		await expect(permissionPopup).not.toBeVisible();
	});
});

test.describe('Claude AskUserQuestion flow', () => {
	test('should show question UI and allow user to answer', async ({ page, context }, testInfo) => {
		const workdir = testInfo.outputPath('workdir');
		const configdir = testInfo.outputPath('config');

		gitbutler = await startGitButler(workdir, configdir, context, {
			CLAUDE_MOCK_SCENARIO: path.join(SCENARIO_DIR, 'ask-user-question.json')
		});

		// Set up a project with a stack to work with
		await gitbutler.runScript('project-with-remote-branches.sh');

		await page.goto('/');

		// Should load the workspace directly (project already set up by script)
		await waitForTestId(page, 'workspace-view');

		// Click the AI button on the existing stack to open codegen view
		await clickByTestId(page, 'branch-header-context-menu-start-codegen-agent');

		// Type a message to Claude that will trigger AskUserQuestion
		const input = getByTestId(page, 'codegen-input');
		await input.locator('div[contenteditable="true"]').fill('Please set up testing');

		// Send the message
		await clickByTestId(page, 'codegen-input-send-button');

		// Wait for the AskUserQuestion UI to appear
		const askUserQuestion = await waitForTestId(page, 'codegen-ask-user-question');
		await expect(askUserQuestion).toBeVisible();

		// Verify the question text is shown
		await expect(askUserQuestion).toContainText('What testing framework would you like to use?');
		await expect(askUserQuestion).toContainText('Jest (Recommended)');

		// Select an option (clicking the first option - Jest)
		const options = page.getByTestId('codegen-ask-user-question-option');
		await options.first().click();

		// Click the Submit button
		await clickByTestId(page, 'codegen-ask-user-question-submit-button');

		// Verify Claude responds with confirmation (this proves the answer was submitted and processed)
		await expect(
			page
				.getByTestId('codegen-messages')
				.getByText("Great choice! I'll set up Jest for your project.")
		).toBeVisible({
			timeout: 10000
		});
	});
});
