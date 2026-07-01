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
import { execFileSync } from "node:child_process";

async function syncAndIntegrate(page: Page) {
	await clickByTestId(page, "sync-button");
	await clickByTestId(page, "integrate-upstream-commits-button");
	await clickByTestId(page, "integrate-upstream-action-button");
}

async function expectWorkspaceCommitParentToBeOriginMaster(pathToRepo: string) {
	await expect
		.poll(() => git(pathToRepo, ["rev-parse", "gitbutler/workspace^"]), {
			message: "Expected the workspace commit to be parented to origin/master",
			intervals: [100, 200, 500, 1000],
		})
		.toBe(git(pathToRepo, ["rev-parse", "origin/master"]));
}

async function expectWorkspaceCommitToStayParentedToRemainingStack(pathToRepo: string) {
	await expect
		.poll(() => git(pathToRepo, ["rev-parse", "gitbutler/workspace^@"]).split("\n").length, {
			message: "Expected the workspace commit to have only the remaining stack as parent",
			intervals: [100, 200, 500, 1000],
		})
		.toBe(1);

	await expect
		.poll(
			() =>
				git(pathToRepo, ["rev-parse", "gitbutler/workspace^"]) ===
				git(pathToRepo, ["rev-parse", "origin/master"]),
			{
				message: "Expected the workspace commit not to be reparented to origin/master",
				intervals: [100, 200, 500, 1000],
			},
		)
		.toBe(false);
}

function git(pathToRepo: string, args: string[]): string {
	return execFileSync("git", args, {
		cwd: pathToRepo,
		encoding: "utf8",
	}).trim();
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

test("should show incoming conflicts with uncommitted files", async ({ page, gitbutler }) => {
	await gitbutler.runScript("project-with-remote-branches.sh");
	await applyUpstream(gitbutler, "branch3");
	await openWorkspace(page);

	await gitbutler.runScript(
		"project-with-remote-branches__add-conflicting-base-and-dirty-worktree.sh",
	);
	await clickByTestId(page, "sync-button");
	await clickByTestId(page, "integrate-upstream-commits-button");

	const worktreeConflicts = await waitForTestId(page, "integrate-upstream-worktree-conflicts");
	await expect(worktreeConflicts).toContainText("a_file");
});

test("should show incoming conflicts with uncommitted files in an empty workspace", async ({
	page,
	gitbutler,
}) => {
	await gitbutler.runScript("project-with-remote-branches.sh");
	await openWorkspace(page);

	await expect(stack(page)).toHaveCount(0);

	await gitbutler.runScript(
		"project-with-remote-branches__add-conflicting-base-and-dirty-worktree.sh",
	);
	await clickByTestId(page, "sync-button");
	await clickByTestId(page, "integrate-upstream-commits-button");

	const worktreeConflicts = await waitForTestId(page, "integrate-upstream-worktree-conflicts");
	await expect(worktreeConflicts).toContainText("a_file");
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

test("should reparent workspace commit to advanced target after integrating all stacks", async ({
	page,
	gitbutler,
}) => {
	const localClone = gitbutler.pathInWorkdir("local-clone");

	await gitbutler.runScript("project-with-remote-branches.sh");
	await applyUpstream(gitbutler, "branch1");
	await openWorkspace(page);

	await expect(stack(page)).toHaveCount(1);

	await gitbutler.runScript(
		"project-with-remote-branches__fast-forward-base-through-branch1-and-add-commit.sh",
	);
	await syncAndIntegrate(page);

	await waitForTestIdToNotExist(page, "stack");
	await expectWorkspaceCommitParentToBeOriginMaster(localClone);
});

test("should reparent workspace commit to advanced merge target after integrating all stacks", async ({
	page,
	gitbutler,
}) => {
	const localClone = gitbutler.pathInWorkdir("local-clone");

	await gitbutler.runScript("project-with-remote-branches.sh");
	await applyUpstream(gitbutler, "branch1");
	await openWorkspace(page);

	await expect(stack(page)).toHaveCount(1);

	await gitbutler.runScript(
		"project-with-remote-branches__merge-branch1-to-base-and-add-commit.sh",
	);
	await syncAndIntegrate(page);

	await waitForTestIdToNotExist(page, "stack");
	await expectWorkspaceCommitParentToBeOriginMaster(localClone);
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

test("should keep the remaining stack when only one of two stacks is integrated", async ({
	page,
	gitbutler,
}) => {
	const localClone = gitbutler.pathInWorkdir("local-clone");

	await gitbutler.runScript("project-with-stacks.sh");
	await applyUpstream(gitbutler, "branch1", "branch2");
	await openWorkspace(page);

	await expect(stack(page)).toHaveCount(2);

	await gitbutler.runScript("merge-upstream-branch-to-base.sh", ["branch1"]);
	await syncAndIntegrate(page);

	await expect(stack(page)).toHaveCount(1);
	await expect(getByTestId(page, "branch-card")).toHaveCount(1);
	await expect(getByTestId(page, "branch-card")).toContainText("branch2");
	await expectWorkspaceCommitToStayParentedToRemainingStack(localClone);
});

test("should handle the update of the workspace with two integrated stacks gracefully", async ({
	page,
	gitbutler,
}) => {
	await gitbutler.runScript("project-with-stacks.sh");
	await applyUpstream(gitbutler, "branch1", "branch2");
	await openWorkspace(page);

	await expect(stack(page)).toHaveCount(2);

	await gitbutler.runScript("merge-upstream-branch-to-base.sh", ["branch1"]);
	await gitbutler.runScript("merge-upstream-branch-to-base.sh", ["branch2"]);
	await syncAndIntegrate(page);

	await expect(stack(page)).toHaveCount(0);
	await waitForTestIdToNotExist(page, "integrate-upstream-commits-button");
});

test("should update an empty workspace when the target ref advances", async ({
	page,
	gitbutler,
}) => {
	const localClone = gitbutler.pathInWorkdir("local-clone");

	await gitbutler.runScript("project-with-stacks.sh");
	await openWorkspace(page);

	await expect(stack(page)).toHaveCount(0);

	await gitbutler.runScript("merge-upstream-branch-to-base.sh", ["branch1"]);
	await syncAndIntegrate(page);

	await expect(stack(page)).toHaveCount(0);
	await expectWorkspaceCommitParentToBeOriginMaster(localClone);
	await waitForTestIdToNotExist(page, "integrate-upstream-commits-button");
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
