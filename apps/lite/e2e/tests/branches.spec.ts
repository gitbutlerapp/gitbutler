import { expect, test } from "../test.ts";
import { execFileSync } from "node:child_process";
import path from "node:path";

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

		const picker = appWindow.getByRole("dialog", { name: "Apply branch" });
		// Hotkeys register in a passive effect, so retry the keypress if the workspace renders first.
		await expect(async () => {
			await appWindow.keyboard.press("ControlOrMeta+Shift+A");
			await expect(picker).toBeVisible({ timeout: 500 });
		}).toPass({ timeout: 5_000 });

		const search = picker.getByRole("combobox", { name: /search for branches/i });
		await search.fill("branch1");
		await picker.getByRole("option", { name: /^branch1 / }).click();

		await expect(picker).toBeHidden();
		await expect(branch).toBeVisible();
		await expect(secondCommit).toBeVisible();
		await expect(firstCommit).toBeVisible();
	});

	test("does not fetch head info after a cached branch creation", async ({
		appWindow,
		mainProcessLogs,
	}) => {
		await expect(appWindow.getByRole("button", { name: "New branch" })).toBeVisible();
		await appWindow.waitForTimeout(1_000);
		const headInfoCalls = () =>
			mainProcessLogs.filter((message) => message.includes("[lite-e2e] headInfo")).length;
		const callsBeforeMutation = headInfoCalls();
		expect(callsBeforeMutation).toBeGreaterThan(0);

		await appWindow.keyboard.press("ControlOrMeta+N");
		await expect(
			appWindow.getByRole("treeitem", { name: "bm-branch-1", exact: true }),
		).toBeVisible();

		// Let the mutation's watcher event settle. Its revision matches the response already cached
		// by the mutation, so it must not trigger another full head-info traversal.
		await appWindow.waitForTimeout(1_000);
		expect(headInfoCalls()).toBe(callsBeforeMutation);
	});

	test("refreshes head info after deleting a packed-only branch", async ({
		appWindow,
		mainProcessLogs,
		testEnvironment,
	}) => {
		await expect(appWindow.getByRole("button", { name: "New branch" })).toBeVisible();
		const repositoryPath = path.join(testEnvironment.workdir, "local-clone");
		const git = (...args: Array<string>) =>
			execFileSync("git", ["-C", repositoryPath, ...args], { encoding: "utf8" });
		const headInfoCalls = () =>
			mainProcessLogs.filter((message) => message.includes("[lite-e2e] headInfo")).length;
		await expect.poll(headInfoCalls).toBeGreaterThan(0);

		await appWindow.keyboard.press("ControlOrMeta+N");
		await expect(
			appWindow.getByRole("treeitem", { name: "bm-branch-1", exact: true }),
		).toBeVisible();

		const callsBeforePacking = headInfoCalls();
		git("pack-refs", "--all", "--prune");
		await expect.poll(headInfoCalls).toBeGreaterThan(callsBeforePacking);
		const callsBeforeDeletion = headInfoCalls();
		git("branch", "-D", "bm-branch-1");

		await expect.poll(headInfoCalls).toBeGreaterThan(callsBeforeDeletion);
		await expect(appWindow.getByRole("treeitem", { name: "bm-branch-1", exact: true })).toHaveCount(
			0,
		);
	});
});
