import { createNewBranch } from "../src/branch.ts";
import { applyUpstream, openWorkspace } from "../src/setup.ts";
import { test } from "../src/test.ts";
import {
	clickByTestId,
	commitRow,
	getByTestId,
	stack,
	waitForTestId,
	waitForTestIdToNotExist,
} from "../src/util.ts";
import { expect, type Page } from "@playwright/test";

async function syncAndIntegrate(page: Page) {
	await clickByTestId(page, "sync-button");
	await clickByTestId(page, "integrate-upstream-commits-button");
	await clickByTestId(page, "integrate-upstream-action-button");
}

test("should handle the update of workspace with one conflicting branch gracefully", async ({
	page,
	gitbutler,
}) => {
	await gitbutler.runScript("project-with-remote-branches.sh");
	await applyUpstream(gitbutler, "branch1");
	await openWorkspace(page);

	await gitbutler.runScript("project-with-remote-branches__add-commit-to-base.sh");
	await syncAndIntegrate(page);

	await expect(stack(page)).toHaveCount(1);
});

test("should handle the update of workspace with integrated branch gracefully", async ({
	page,
	gitbutler,
}) => {
	await gitbutler.runScript("project-with-remote-branches.sh");
	await applyUpstream(gitbutler, "branch1");
	await openWorkspace(page);

	await expect(stack(page)).toHaveCount(1);

	await gitbutler.runScript("merge-upstream-branch-to-base.sh", ["branch1"]);
	await syncAndIntegrate(page);

	await waitForTestIdToNotExist(page, "stack");
});

test("should handle the update of workspace with integrated parent branch in stack gracefully", async ({
	page,
	gitbutler,
}) => {
	await gitbutler.runScript("project-with-remote-branches.sh");
	await applyUpstream(gitbutler, "branch1", "branch3");
	await gitbutler.runScript("move-branch.sh", ["branch3", "branch1", "local-clone"]);
	await openWorkspace(page);

	await expect(stack(page)).toHaveCount(1);
	await expect(getByTestId(page, "branch-card")).toHaveCount(2);

	await gitbutler.runScript("merge-upstream-branch-to-base.sh", ["branch1"]);
	await clickByTestId(page, "sync-button");
	await clickByTestId(page, "integrate-upstream-commits-button");

	const branch1Row = page.locator('[data-integration-row-branch-name="branch1"]').first();
	const statusBadge = branch1Row.getByTestId("integrate-upstream-series-row-status-badge");
	await expect(statusBadge).toHaveText("Integrated");

	await clickByTestId(page, "integrate-upstream-action-button");

	await expect(stack(page)).toHaveCount(1);
	await expect(getByTestId(page, "branch-card")).toHaveCount(1);
});

test("should handle the update of the workspace with multiple stacks gracefully", async ({
	page,
	gitbutler,
}) => {
	await gitbutler.runScript("project-with-stacks.sh");
	await applyUpstream(gitbutler, "branch1", "branch2");
	await openWorkspace(page);

	await expect(stack(page)).toHaveCount(2);

	await gitbutler.runScript("merge-upstream-branch-to-base.sh", ["branch1"]);
	await syncAndIntegrate(page);

	await expect(stack(page)).toHaveCount(1);
});

test("should handle the update of an empty branch gracefully", async ({ page, gitbutler }) => {
	await gitbutler.runScript("project-with-stacks.sh");
	await openWorkspace(page);

	await expect(stack(page)).toHaveCount(0);
	await createNewBranch(page, "new-branch");
	await expect(stack(page)).toHaveCount(1);

	await gitbutler.runScript("merge-upstream-branch-to-base.sh", ["branch1"]);
	await syncAndIntegrate(page);

	await expect(stack(page)).toHaveCount(1);
});

test("should handle the update of a branch with an empty commit", async ({ page, gitbutler }) => {
	await gitbutler.runScript("project-with-stacks.sh");
	await openWorkspace(page);

	await expect(stack(page)).toHaveCount(0);

	// Create a new branch
	await clickByTestId(page, "chrome-header-create-branch-button");
	const modal = await waitForTestId(page, "create-new-branch-modal");
	await modal.locator("#new-branch-name-input").fill("new-branch");
	await clickByTestId(page, "confirm-submit");
	await expect(stack(page)).toHaveCount(1);

	// Add an empty commit via the branch context menu.
	const branchCard = getByTestId(page, "branch-card");
	await branchCard.click({ button: "right" });
	await waitForTestId(page, "branch-header-context-menu");
	await clickByTestId(page, "branch-header-context-menu-add-empty-commit");
	await expect(commitRow(page)).toHaveCount(1);

	await gitbutler.runScript("merge-upstream-branch-to-base.sh", ["branch1"]);
	await syncAndIntegrate(page);
	await waitForTestIdToNotExist(page, "integrate-upstream-action-button");

	await expect(stack(page)).toHaveCount(1);
	await expect(commitRow(page)).toHaveCount(1);
});
