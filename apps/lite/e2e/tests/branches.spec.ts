import { execFileSync } from "node:child_process";
import path from "node:path";
import type { HeadAndMode } from "@gitbutler/but-sdk";
import type { ElectronApplication, Page } from "@playwright/test";
import { expect, test } from "../test.ts";

type NativeMenuItem = {
	_tag: "Item" | "Separator";
	enabled?: boolean;
	itemId?: string;
	label?: string;
};

const checkedOutBranchName = async (appWindow: Page): Promise<string | null> =>
	appWindow.evaluate(async () => {
		const projectId = location.pathname.split("/")[2];
		if (projectId === undefined) return null;
		const lite = (
			window as unknown as {
				lite: { operatingMode: (projectId: string) => Promise<HeadAndMode> };
			}
		).lite;
		const { operatingMode } = await lite.operatingMode(projectId);
		return operatingMode.type === "OutsideWorkspace" ? operatingMode.subject.branchName : null;
	});

const chooseNewBranchAction = async (
	electronApp: ElectronApplication,
	appWindow: Page,
	label: string,
): Promise<Array<NativeMenuItem>> => {
	await electronApp.evaluate(({ ipcMain }, selectedLabel) => {
		const state = globalThis as typeof globalThis & {
			newBranchMenuItems?: Array<NativeMenuItem>;
		};
		delete state.newBranchMenuItems;
		ipcMain.removeHandler("showNativeMenu");
		ipcMain.handle("showNativeMenu", (_event, params: { items: Array<NativeMenuItem> }) => {
			state.newBranchMenuItems = params.items;
			const item = params.items.find((item) => item.label === selectedLabel);
			return item?.enabled === false ? null : (item?.itemId ?? null);
		});
	}, label);

	await appWindow.getByRole("button", { name: "New branch" }).click();
	const readItems = () =>
		electronApp.evaluate(() => {
			const state = globalThis as typeof globalThis & {
				newBranchMenuItems?: Array<NativeMenuItem>;
			};
			return state.newBranchMenuItems ?? [];
		});
	await expect.poll(readItems).not.toEqual([]);
	return readItems();
};

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

test.describe("new branch outside the workspace", () => {
	test.use({ scenario: "project-with-master-checked-out.sh" });

	test("only offers branch checkout when master is checked out", async ({
		appWindow,
		electronApp,
	}) => {
		await expect.poll(() => checkedOutBranchName(appWindow)).toBe("refs/heads/master");
		const menuItems = await chooseNewBranchAction(
			electronApp,
			appWindow,
			"New Branch in Workspace",
		);
		expect(menuItems.find((item) => item.label === "New Branch in Workspace")?.enabled).toBe(false);
		expect(menuItems.find((item) => item.label === "New Branch and Switch to It")?.enabled).toBe(
			true,
		);

		await chooseNewBranchAction(electronApp, appWindow, "New Branch and Switch to It");
		await expect
			.poll(async () => {
				const checkedOut = await checkedOutBranchName(appWindow);
				return checkedOut !== null && checkedOut !== "" && checkedOut !== "refs/heads/master";
			})
			.toBe(true);
	});
});

test.describe("branches", () => {
	test.use({ scenario: "project-with-remote-branches.sh" });

	test("applies a remote branch to the workspace", async ({ appWindow }) => {
		await expect(appWindow.getByRole("combobox", { name: /select project/i })).toBeVisible();

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

test.describe("workspace branch creation", () => {
	test.use({ scenario: "project-with-diverged-branch-and-parallel-empty-branch.sh" });

	test("creates an independent branch in an ordinary workspace", async ({
		appWindow,
		electronApp,
	}) => {
		await expect(
			appWindow.getByRole("treeitem", { name: "empty-branch", exact: true }),
		).toBeVisible();
		const stacks = appWindow.getByRole("group", { name: "Stack" });
		const initialStackCount = await stacks.count();
		const menuItems = await chooseNewBranchAction(
			electronApp,
			appWindow,
			"New Branch in Workspace",
		);
		expect(menuItems.find((item) => item.label === "New Branch in Workspace")?.enabled).toBe(true);
		await expect(stacks).toHaveCount(initialStackCount + 1);
	});
});
