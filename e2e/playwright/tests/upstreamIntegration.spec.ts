import { createNewBranch } from "../src/branch.ts";
import { BUT } from "../src/env.ts";
import { getBaseURL, type GitButler, startGitButler } from "../src/setup.ts";
import { test } from "../src/test.ts";
import { clickByTestId, getByTestId, waitForTestId, waitForTestIdToNotExist } from "../src/util.ts";
import { expect, type Page } from "@playwright/test";
import { execSync } from "child_process";

/** Open the integrate-upstream modal: sync, then click the button. */
async function openIntegrateModal(page: Page) {
	await clickByTestId(page, "sync-button");
	await clickByTestId(page, "integrate-upstream-commits-button");
	await waitForTestId(page, "integrate-upstream-commits-modal");
}

/**
 * Find an integration series row by branch name and return a helper
 * object with a locator and a resolution select action.
 */
function integrationRow(page: Page, branchName: string) {
	// Find the series row that contains this branch name as text
	const row = page.getByTestId("integrate-upstream-series-row").filter({ hasText: branchName });
	return {
		row,
		async selectResolution(label: string) {
			// Click the select trigger (textbox showing current resolution)
			const selectTrigger = row.getByRole("textbox");
			await selectTrigger.click();
			// Pick the option from the listbox
			const option = page.getByRole("listbox").getByText(label, { exact: true });
			await option.click();
		},
	};
}

/** Get the parent of the bottom commit for a branch (its effective base). */
function getBranchBase(projectDir: string, configDir: string, branchName: string): string {
	const raw = execSync(`${BUT} status --json`, {
		cwd: projectDir,
		encoding: "utf-8",
		env: { ...process.env, E2E_TEST_APP_DATA_DIR: configDir },
	});
	const status = JSON.parse(raw);
	for (const stack of status.stacks) {
		for (const branch of stack.branches) {
			if (branch.name === branchName) {
				const bottomCommit = branch.commits[branch.commits.length - 1];
				// Get the parent of the bottom commit — this is the branch's base
				return execSync(`git rev-parse ${bottomCommit.commitId}^`, {
					cwd: projectDir,
					encoding: "utf-8",
				}).trim();
			}
		}
	}
	throw new Error(`Branch "${branchName}" not found in but status output`);
}

let gitbutler: GitButler;

test.use({
	baseURL: getBaseURL(),
});

test.afterEach(async () => {
	await gitbutler?.destroy();
});

test("should handle the update of workspace with one conflicting branch gracefully", async ({
	page,
	context,
}, testInfo) => {
	const workdir = testInfo.outputPath("workdir");
	const configdir = testInfo.outputPath("config");
	gitbutler = await startGitButler(workdir, configdir, context);

	await gitbutler.runScript("project-with-remote-branches.sh");
	// Apply branch1
	await gitbutler.runScript("apply-upstream-branch.sh", ["branch1", "local-clone"]);

	await page.goto("/");

	// Should load the workspace
	await waitForTestId(page, "workspace-view");

	// There are remote changes in the base branch
	await gitbutler.runScript("project-with-remote-branches__add-commit-to-base.sh");

	// Click the sync button
	await clickByTestId(page, "sync-button");

	// Update the workspace
	await clickByTestId(page, "integrate-upstream-commits-button");
	await clickByTestId(page, "integrate-upstream-action-button");

	// There should be one stack
	const stacks = getByTestId(page, "stack");
	await expect(stacks).toHaveCount(1);
});

test("should handle the update of workspace with integrated branch gracefully", async ({
	page,
	context,
}, testInfo) => {
	const workdir = testInfo.outputPath("workdir");
	const configdir = testInfo.outputPath("config");
	gitbutler = await startGitButler(workdir, configdir, context);

	await gitbutler.runScript("project-with-remote-branches.sh");
	// Apply branch1
	await gitbutler.runScript("apply-upstream-branch.sh", ["branch1", "local-clone"]);

	await page.goto("/");

	// Should load the workspace
	await waitForTestId(page, "workspace-view");

	// There should be one stack applied
	const stacks = getByTestId(page, "stack");
	await expect(stacks).toHaveCount(1);

	// Branch one was merged in the forge
	await gitbutler.runScript("merge-upstream-branch-to-base.sh", ["branch1"]);

	// Click the sync button
	await clickByTestId(page, "sync-button");

	// Update the workspace
	await clickByTestId(page, "integrate-upstream-commits-button");
	await clickByTestId(page, "integrate-upstream-action-button");

	// There should be no stacks
	await waitForTestIdToNotExist(page, "stack");
});

test("should handle the update of workspace with integrated parent branch in stack gracefully", async ({
	page,
	context,
}, testInfo) => {
	const workdir = testInfo.outputPath("workdir");
	const configdir = testInfo.outputPath("config");
	gitbutler = await startGitButler(workdir, configdir, context);

	await gitbutler.runScript("project-with-remote-branches.sh");
	// Apply branch1
	await gitbutler.runScript("apply-upstream-branch.sh", ["branch1", "local-clone"]);
	await gitbutler.runScript("apply-upstream-branch.sh", ["branch3", "local-clone"]);
	await gitbutler.runScript("move-branch.sh", ["branch3", "branch1", "local-clone"]);

	await page.goto("/");

	// Should load the workspace
	await waitForTestId(page, "workspace-view");

	// There should be one stack applied
	let stacks = getByTestId(page, "stack");
	await expect(stacks).toHaveCount(1);
	let branchCards = getByTestId(page, "branch-card");
	await expect(branchCards).toHaveCount(2);

	// Branch one was merged in the forge
	await gitbutler.runScript("merge-upstream-branch-to-base.sh", ["branch1"]);

	// Click the sync button
	await clickByTestId(page, "sync-button");

	// Update the workspace
	await clickByTestId(page, "integrate-upstream-commits-button");

	// The status of the branch1 should be "Integrated"
	const branch1Status = page.locator('[data-integration-row-branch-name="branch1"]').first();
	await branch1Status.waitFor();
	const statusBadge = branch1Status.getByTestId("integrate-upstream-series-row-status-badge");
	await statusBadge.waitFor();
	await expect(statusBadge).toHaveText("Integrated");

	await clickByTestId(page, "integrate-upstream-action-button");

	// There should be one stack left with one branch
	stacks = getByTestId(page, "stack");
	await expect(stacks).toHaveCount(1);
	branchCards = getByTestId(page, "branch-card");
	await expect(branchCards).toHaveCount(1);
});

test("should handle the update of the workspace with multiple stacks gracefully", async ({
	page,
	context,
}, testInfo) => {
	const workdir = testInfo.outputPath("workdir");
	const configdir = testInfo.outputPath("config");
	gitbutler = await startGitButler(workdir, configdir, context);

	await gitbutler.runScript("project-with-stacks.sh");
	await gitbutler.runScript("apply-upstream-branch.sh", ["branch1", "local-clone"]);
	await gitbutler.runScript("apply-upstream-branch.sh", ["branch2", "local-clone"]);

	await page.goto("/");

	// Should load the workspace
	await waitForTestId(page, "workspace-view");

	// There should be one stack applied
	let stacks = getByTestId(page, "stack");
	await expect(stacks).toHaveCount(2);

	// Branch one was merged in the forge
	await gitbutler.runScript("merge-upstream-branch-to-base.sh", ["branch1"]);

	// Click the sync button
	await clickByTestId(page, "sync-button");

	// Update the workspace
	await clickByTestId(page, "integrate-upstream-commits-button");
	await clickByTestId(page, "integrate-upstream-action-button");

	// There should be one stack left
	stacks = getByTestId(page, "stack");
	await expect(stacks).toHaveCount(1);
});

test("should handle the update of an empty branch gracefully", async ({
	page,
	context,
}, testInfo) => {
	const workdir = testInfo.outputPath("workdir");
	const configdir = testInfo.outputPath("config");
	gitbutler = await startGitButler(workdir, configdir, context);

	await gitbutler.runScript("project-with-stacks.sh");

	await page.goto("/");

	// Should load the workspace
	await waitForTestId(page, "workspace-view");

	// There should be no stacks
	let stacks = getByTestId(page, "stack");
	await expect(stacks).toHaveCount(0);

	// Create a new branch
	await createNewBranch(page, "new-branch");

	// There should be no stacks
	stacks = getByTestId(page, "stack");
	await expect(stacks).toHaveCount(1);

	// Branch one was merged in the forge
	await gitbutler.runScript("merge-upstream-branch-to-base.sh", ["branch1"]);

	// Click the sync button
	await clickByTestId(page, "sync-button");

	// Update the workspace
	await clickByTestId(page, "integrate-upstream-commits-button");
	await clickByTestId(page, "integrate-upstream-action-button");

	// There should be one stack left
	stacks = getByTestId(page, "stack");
	await expect(stacks).toHaveCount(1);
});

test("should handle the update of a branch with an empty commit", async ({
	page,
	context,
}, testInfo) => {
	const workdir = testInfo.outputPath("workdir");
	const configdir = testInfo.outputPath("config");
	gitbutler = await startGitButler(workdir, configdir, context);

	await gitbutler.runScript("project-with-stacks.sh");

	await page.goto("/");

	// Should load the workspace
	await waitForTestId(page, "workspace-view");

	// There should be no stacks
	let stacks = getByTestId(page, "stack");
	await expect(stacks).toHaveCount(0);

	// Create a new branch
	await clickByTestId(page, "chrome-header-create-branch-button");
	const modal = await waitForTestId(page, "create-new-branch-modal");

	const input = modal.locator("#new-branch-name-input");
	await input.fill("new-branch");
	await clickByTestId(page, "confirm-submit");

	// There should be one stack
	stacks = getByTestId(page, "stack");
	await expect(stacks).toHaveCount(1);

	const branchCard = getByTestId(page, "branch-card");
	await branchCard.isVisible();
	await branchCard.click({
		button: "right",
	});

	// Add an empty commit
	await waitForTestId(page, "branch-header-context-menu");
	await clickByTestId(page, "branch-header-context-menu-add-empty-commit");

	// There should be one commit
	let commits = getByTestId(page, "commit-row");
	await expect(commits).toHaveCount(1);

	// Branch one was merged in the forge
	await gitbutler.runScript("merge-upstream-branch-to-base.sh", ["branch1"]);

	// Click the sync button
	await clickByTestId(page, "sync-button");

	// Update the workspace
	await clickByTestId(page, "integrate-upstream-commits-button");
	await clickByTestId(page, "integrate-upstream-action-button");

	await waitForTestIdToNotExist(page, "integrate-upstream-action-button");

	// There should be one stack left
	stacks = getByTestId(page, "stack");
	await expect(stacks).toHaveCount(1);
	// There should be one commit
	commits = getByTestId(page, "commit-row");
	await expect(commits).toHaveCount(1);
});

test("should leave a branch as-is when 'Leave as is' is selected", async ({
	page,
	context,
}, testInfo) => {
	const workdir = testInfo.outputPath("workdir");
	const configdir = testInfo.outputPath("config");
	gitbutler = await startGitButler(workdir, configdir, context);

	// Setup: two independent stacks (branch1 and branch2)
	await gitbutler.runScript("project-with-stacks.sh");
	await gitbutler.runScript("apply-upstream-branch.sh", ["branch1", "local-clone"]);
	await gitbutler.runScript("apply-upstream-branch.sh", ["branch2", "local-clone"]);

	await page.goto("/");
	await waitForTestId(page, "workspace-view");

	// Both stacks are applied
	await expect(getByTestId(page, "stack")).toHaveCount(2);

	const projectDir = gitbutler.pathInWorkdir("local-clone");

	// Capture the old target base before upstream changes
	const oldBase = execSync("git rev-parse origin/master", {
		cwd: projectDir,
		encoding: "utf-8",
	}).trim();

	// Add upstream commits so both branches are behind
	await gitbutler.runScript("merge-upstream-branch-to-base.sh", ["branch3"]);

	// Fetch so origin/master is updated in the local clone
	execSync("git fetch origin", { cwd: projectDir });

	// Capture the new target base after upstream merge
	const newBase = execSync("git rev-parse origin/master", {
		cwd: projectDir,
		encoding: "utf-8",
	}).trim();
	expect(newBase).not.toEqual(oldBase);

	// Open the modal
	await openIntegrateModal(page);

	// Find branch1's row and change it to "Leave as is"
	const branch1 = integrationRow(page, "branch1");
	await branch1.selectResolution("Leave as is");

	// Click integrate
	await clickByTestId(page, "integrate-upstream-action-button");
	await waitForTestId(page, "workspace-view");

	// Both stacks should still be present — branch1 was left as-is
	await expect(getByTestId(page, "stack")).toHaveCount(2);

	// branch1 was left as-is: its base should still be the old target
	const branch1Base = getBranchBase(projectDir, configdir, "branch1");
	expect(branch1Base).toEqual(oldBase);

	// branch2 was rebased: its base should now be the new target
	const branch2Base = getBranchBase(projectDir, configdir, "branch2");
	expect(branch2Base).toEqual(newBase);
});
