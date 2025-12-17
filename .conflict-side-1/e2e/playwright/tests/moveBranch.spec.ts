import { getBaseURL, type GitButler, startGitButler } from '../src/setup.ts';
import { dragAndDropByLocator, sleep, waitForTestId } from '../src/util.ts';
import { expect, test } from '@playwright/test';

let gitbutler: GitButler;

test.use({
	baseURL: getBaseURL()
});

test.afterEach(async () => {
	await gitbutler?.destroy();
});

test('move branch to top of other stack and tear it off', async ({ page, context }, testInfo) => {
	const workdir = testInfo.outputPath('workdir');
	const configdir = testInfo.outputPath('config');
	gitbutler = await startGitButler(workdir, configdir, context);

	await gitbutler.runScript('project-with-stacks.sh');
	await gitbutler.runScript('apply-upstream-branch.sh', ['branch1', 'local-clone']);
	await gitbutler.runScript('apply-upstream-branch.sh', ['branch2', 'local-clone']);

	await page.goto('/');

	// Should load the workspace
	await waitForTestId(page, 'workspace-view');

	let stacks = page.getByTestId('stack');
	await expect(stacks).toHaveCount(2);
	const stack1 = stacks.filter({ hasText: 'branch1' });
	await stack1.isVisible();
	const stack2 = stacks.filter({ hasText: 'branch2' });
	await stack2.isVisible();

	let branchHeaders = page.getByTestId('branch-header');
	await expect(branchHeaders).toHaveCount(2);
	const branch1Locator = branchHeaders.filter({ hasText: 'branch1' });
	const branch2Locator = branchHeaders.filter({ hasText: 'branch2' });

	// Drag branch1 to the top of branch2's stack
	// The dropzone above branch2 (isFirst=true) activates on hover during drag
	// We drag to branch2 with a position offset to hit the dropzone area above it
	await dragAndDropByLocator(page, branch1Locator, branch2Locator, {
		force: true,
		position: {
			x: 120,
			y: -10
		}
	});

	// Should have moved branch1 to the top of stack2
	stacks = page.getByTestId('stack');
	await expect(stacks).toHaveCount(1);
	branchHeaders = page.getByTestId('branch-header');
	await expect(branchHeaders).toHaveCount(2);

	// Now tear off branch2
	const updatedBranch1Locator = branchHeaders.filter({ hasText: 'branch2' });
	const stackDropzone = await waitForTestId(page, 'stack-offlane-dropzone');
	await dragAndDropByLocator(page, updatedBranch1Locator, stackDropzone, {
		force: true,
		position: {
			x: 10,
			y: 10
		}
	});

	// Should have two stacks again
	stacks = page.getByTestId('stack');
	await expect(stacks).toHaveCount(2);
	branchHeaders = page.getByTestId('branch-header');
	await expect(branchHeaders).toHaveCount(2);
});

test('move branch to the middle of other stack', async ({ page, context }, testInfo) => {
	const workdir = testInfo.outputPath('workdir');
	const configdir = testInfo.outputPath('config');
	gitbutler = await startGitButler(workdir, configdir, context);

	await gitbutler.runScript('project-with-stacks.sh');
	await gitbutler.runScript('apply-upstream-branch.sh', ['branch1', 'local-clone']);
	await gitbutler.runScript('apply-upstream-branch.sh', ['branch2', 'local-clone']);
	await gitbutler.runScript('apply-upstream-branch.sh', ['branch3', 'local-clone']);

	await page.goto('/');

	// Should load the workspace
	await waitForTestId(page, 'workspace-view');

	let stacks = page.getByTestId('stack');
	await expect(stacks).toHaveCount(3);
	const stack1 = stacks.filter({ hasText: 'branch1' });
	await stack1.isVisible();
	const stack2 = stacks.filter({ hasText: 'branch2' });
	await stack2.isVisible();
	const stack3 = stacks.filter({ hasText: 'branch3' });
	await stack3.isVisible();

	let branchHeaders = page.getByTestId('branch-header');
	await expect(branchHeaders).toHaveCount(3);
	let branch1Locator = branchHeaders.filter({ hasText: 'branch1' });
	const branch2Locator = branchHeaders.filter({ hasText: 'branch2' });

	// Move branch 2 on top of branch 1
	// Drag to branch1 with position offset to hit the dropzone above it
	await dragAndDropByLocator(page, branch2Locator, branch1Locator, {
		force: true,
		position: {
			x: 120,
			y: -10
		}
	});
	stacks = page.getByTestId('stack');
	await expect(stacks).toHaveCount(2);

	await sleep(500); // It seems that we need to wait a bit for the DOM to stabilize

	branchHeaders = page.getByTestId('branch-header');
	await expect(branchHeaders).toHaveCount(3);
	// Move branch3 on top of branch 1 (which is now in the middle of stack)
	const branch3Locator = branchHeaders.filter({ hasText: 'branch3' });
	branch1Locator = branchHeaders.filter({ hasText: 'branch1' });

	// After merge, there's one stack with branch2 on top and branch1 below
	// Drag to branch1 with position offset to hit the dropzone above it
	await dragAndDropByLocator(page, branch3Locator, branch1Locator, {
		force: true,
		position: {
			x: 120,
			y: -10
		}
	});

	// Should have moved branch1 to the top of stack2
	stacks = page.getByTestId('stack');
	await expect(stacks).toHaveCount(1);
	branchHeaders = page.getByTestId('branch-header');
	await expect(branchHeaders).toHaveCount(3);
});
