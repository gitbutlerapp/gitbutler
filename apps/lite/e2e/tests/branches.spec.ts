import { execFileSync } from "node:child_process";
import path from "node:path";
import type { Page } from "@playwright/test";
import { expect, test } from "../test.ts";

const applyBranch = async (appWindow: Page, name: string) => {
	const picker = appWindow.getByRole("dialog", { name: "Apply branch" });
	await expect(async () => {
		await appWindow.keyboard.press("ControlOrMeta+Shift+A");
		await expect(picker).toBeVisible({ timeout: 500 });
	}).toPass({ timeout: 5_000 });

	const search = picker.getByRole("combobox", { name: /search for branches/i });
	await search.fill(name);
	await picker.getByRole("option", { name: new RegExp(`^${name} `) }).click();
	await expect(picker).toBeHidden();
};

test.describe("branches", () => {
	test.use({ scenario: "project-with-remote-branches.sh" });

	test("applies a remote branch to the workspace", async ({ appWindow }) => {
		await expect(appWindow.getByRole("button", { name: /select project/i })).toBeVisible();

		const branch = appWindow.getByRole("treeitem", { name: "branch1", exact: true });
		const secondCommit = appWindow.getByRole("treeitem", { name: "branch1: second commit" });
		const firstCommit = appWindow.getByRole("treeitem", { name: "branch1: first commit" });
		await expect(branch).toHaveCount(0);
		await expect(secondCommit).toHaveCount(0);
		await expect(firstCommit).toHaveCount(0);

		await applyBranch(appWindow, "branch1");
		await expect(branch).toBeVisible();
		await expect(secondCommit).toBeVisible();
		await expect(firstCommit).toBeVisible();
	});

	test("shows one error when renaming onto an existing branch", async ({ appWindow }) => {
		await applyBranch(appWindow, "branch1");

		const branch = appWindow.getByRole("treeitem", { name: "branch1", exact: true });
		await branch.getByTitle("branch1").click();
		await appWindow.keyboard.press("F2");
		const editor = appWindow.getByRole("textbox", { name: "Branch name" });
		await editor.fill("master");
		await editor.press("Enter");

		// Base UI renders each title once in the toast and once in its live region.
		await expect(appWindow.getByText("Failed to rename branch", { exact: true })).toHaveCount(2);
		await expect(branch).toBeVisible();
	});

	test("removes a deleted branch from the branches view", async ({
		appWindow,
		testEnvironment,
	}) => {
		const repositoryPath = path.join(testEnvironment.workdir, "local-clone");
		execFileSync("git", ["-C", repositoryPath, "branch", "delete-me", "origin/branch2"]);

		const pages = appWindow.getByRole("group", { name: "Pages" });
		await pages.getByRole("button", { name: "Branches" }).click();
		const branch = appWindow.getByRole("treeitem", { name: "delete-me", exact: true });
		await expect(branch).toBeVisible();

		await appWindow.evaluate(() =>
			(
				window as unknown as { lite: { watcherStopAll: () => Promise<number> } }
			).lite.watcherStopAll(),
		);
		await branch.click();
		await appWindow.keyboard.press(process.platform === "darwin" ? "Meta+Backspace" : "Delete");

		await expect(branch).toHaveCount(0);
	});
});
