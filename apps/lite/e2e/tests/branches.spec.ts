import type { App } from "../app.ts";
import { expect, test } from "../test.ts";

const newBranch = (app: App) => app.getByRole("button", { name: "New branch" });

test.describe("new branch outside the workspace", () => {
	test.use({ scenario: "project-with-master-checked-out.sh" });

	test("can't be created in the workspace", async ({ app }) => {
		const items = await app.chooseFromNativeMenu(newBranch(app), "New Branch in Workspace");

		expect(items).toContainEqual(
			expect.objectContaining({ label: "New Branch in Workspace", enabled: false }),
		);
	});

	test("switches to a new branch", async ({ app }) => {
		await app.chooseFromNativeMenu(newBranch(app), "New Branch and Switch to It");

		await expect.poll(() => app.checkedOutBranch()).not.toBe("refs/heads/master");
	});
});

test.describe("branches", () => {
	test.use({ scenario: "project-with-remote-branches.sh" });

	test("applies a remote branch to the workspace", async ({ app }) => {
		await app.applyBranch("branch1");

		await expect(app.branch("branch1")).toBeVisible();
		await expect(app.commit("branch1: second commit")).toBeVisible();
		await expect(app.commit("branch1: first commit")).toBeVisible();
	});

	test("shows one error when renaming onto an existing branch", async ({ app }) => {
		await app.applyBranch("branch1");

		const editor = await app.editBranchName("branch1");
		await editor.fill("master");
		await editor.press("Enter");

		// Base UI renders each title once in the toast and once in its live region.
		await expect(app.getByText("Failed to rename branch", { exact: true })).toHaveCount(2);
		await expect(app.branch("branch1")).toBeVisible();
	});

	test("removes a deleted branch from the branches view", async ({ app, repo }) => {
		await repo.createBranch("delete-me", "origin/branch2");
		await app.goTo("Branches");
		await expect(app.branch("delete-me")).toBeVisible();
		// Only the app's own update may remove the row, not the watcher noticing the ref go.
		await app.stopWatchingFiles();

		await app.branch("delete-me").click();
		await app.pressDelete();

		await expect(app.branch("delete-me")).toHaveCount(0);
	});
});

test.describe("workspace branch creation", () => {
	test.use({ scenario: "project-with-diverged-branch-and-parallel-empty-branch.sh" });

	test("creates an independent branch in an ordinary workspace", async ({ app }) => {
		await expect(app.branch("empty-branch")).toBeVisible();
		const stacks = app.getByRole("group", { name: "Stack" });
		const stackCount = await stacks.count();

		await app.chooseFromNativeMenu(newBranch(app), "New Branch in Workspace");

		await expect(stacks).toHaveCount(stackCount + 1);
	});
});
