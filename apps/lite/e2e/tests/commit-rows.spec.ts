import type { LiteElectronApi } from "../../electron/src/ipc.ts";
import { divergeBranch1 } from "../diverge.ts";
import { expect, test } from "../test.ts";

test.describe("workspace commit rows", () => {
	test.use({ scenario: "project-in-single-branch-three-branch-stack.sh" });

	test("shows metadata without moving the label on hover or losing inline editing", async ({
		appWindow,
	}) => {
		const row = appWindow.getByRole("treeitem", { name: "B: first commit", exact: true });
		await expect(row).toBeVisible();
		await expect(row).toHaveAccessibleDescription(/Branchy McBranchface.*[0-9a-f]{7}/);
		await expect(row.getByText("Branchy McBranchface", { exact: true })).toBeVisible();
		expect((await row.boundingBox())?.height).toBe(54);
		await appWindow.mouse.move(0, 0);
		const label = row.getByText("B: first commit", { exact: true }).locator("..");
		const before = await label.boundingBox();
		await row.hover();
		await expect(row.getByRole("button", { name: "Commit menu", exact: true })).toBeVisible();
		expect(await label.boundingBox()).toEqual(before);

		await row.getByText("B: first commit", { exact: true }).dblclick();
		const editor = row.getByRole("textbox", { name: "Commit message" });
		await expect(editor).toBeFocused();
		await editor.fill("An unsaved message");
		await editor.press("Escape");
		await expect(row.getByText("B: first commit", { exact: true })).toBeVisible();
		await expect(row).toHaveAccessibleDescription(/Branchy McBranchface.*[0-9a-f]{7}/);
	});
});

test.describe("unapplied commit rows", () => {
	test.use({ scenario: "project-with-remote-branches.sh" });

	test("shows the same metadata and keeps the menu out of the label's space", async ({
		appWindow,
	}) => {
		await appWindow
			.getByRole("group", { name: "Pages" })
			.getByRole("button", { name: "Branches", exact: true })
			.click();
		const branch = appWindow.getByRole("treeitem", { name: "branch1", exact: true });
		await branch.getByRole("button", { name: "Unfold commits" }).click();
		const row = branch.getByRole("treeitem", { name: "branch1: first commit", exact: true });
		await expect(row).toHaveAccessibleDescription(/Branchy McBranchface.*[0-9a-f]{7}/);
		await expect(row.getByText("Branchy McBranchface", { exact: true })).toBeVisible();
		expect((await row.boundingBox())?.height).toBe(54);
		await appWindow.mouse.move(0, 0);
		const label = row.getByText("branch1: first commit", { exact: true }).locator("..");
		const before = await label.boundingBox();
		await row.hover();
		await expect(row.getByRole("button", { name: "Commit menu", exact: true })).toBeVisible();
		expect(await label.boundingBox()).toEqual(before);
		await row.click();
		await expect(row).toHaveAttribute("aria-selected", "true");
	});
});

test.describe("incoming commit rows", () => {
	test.use({ scenario: "project-with-remote-branches.sh" });
	test("shows metadata for incoming commits", async ({ appWindow, testEnvironment }) => {
		divergeBranch1(testEnvironment);
		await appWindow.reload();
		await appWindow
			.getByRole("button", { name: "Show 2 incoming commits from origin/branch1", exact: true })
			.click();
		const content = appWindow
			.getByText("Document the reworked entry point", { exact: true })
			.locator("..")
			.locator("..");
		await expect(content.getByText("Branchy McBranchface", { exact: true })).toBeVisible();
		await expect(content.getByText(/^[0-9a-f]{7}$/)).toBeVisible();
	});
});

test.describe("upstream history commit rows", () => {
	test.use({ scenario: "project-in-single-branch-three-branch-stack.sh" });
	test("uses the review title and shows authored time and short SHA", async ({
		appWindow,
		electronApp,
	}) => {
		const listing = await appWindow.evaluate(async () => {
			const projectId = location.pathname.split("/")[2];
			if (projectId === undefined) throw new Error("No project");
			return (window as unknown as { lite: LiteElectronApi }).lite.workspaceTargetCommits({
				projectId,
				from: null,
				limit: null,
			});
		});
		const target = listing.commits[0];
		if (target === undefined) throw new Error("No target commit");
		const enriched = {
			...listing,
			commits: [
				{
					...target,
					commit: {
						...target.commit,
						authoredAt: Date.now() - 86_400_000,
						committedAt: Date.now(),
					},
					review: {
						number: 42,
						title: "Improve parser diagnostics",
						htmlUrl: "https://example.com/pull/42",
						unitSymbol: "#",
						sourceBranch: "parser-diagnostics",
					},
				},
				...listing.commits.slice(1),
			],
		};
		await electronApp.evaluate(({ ipcMain }, data) => {
			ipcMain.removeHandler("workspaceTargetCommits");
			ipcMain.handle("workspaceTargetCommits", (_event, { from }: { from: string | null }) =>
				from === null ? data : { commits: [], hasMore: false },
			);
		}, enriched);
		await appWindow.reload();
		await appWindow.getByRole("button", { name: "Unfold history", exact: true }).click();
		const row = appWindow.getByRole("treeitem", {
			name: "Improve parser diagnostics",
			exact: true,
		});
		await expect(row.getByText("Branchy McBranchface", { exact: true })).toBeVisible();
		await expect(row.getByText("1 day ago", { exact: true })).toBeVisible();
		await expect(row.getByText(target.commit.id.slice(0, 7), { exact: true })).toBeVisible();
		await expect(row).toHaveAccessibleDescription(/Branchy McBranchface.*1 day ago.*[0-9a-f]{7}/);
	});
});
