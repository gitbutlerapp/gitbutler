import type { LiteElectronApi } from "../../electron/src/ipc.ts";
import { expect, test } from "../test.ts";

test.describe("workspace commit rows", () => {
	test.use({ scenario: "project-in-single-branch-three-branch-stack.sh" });

	test("shows a single line without moving the label on hover or losing inline editing", async ({
		appWindow,
	}) => {
		const row = appWindow.getByRole("treeitem", { name: "B: first commit", exact: true });
		await expect(row).toBeVisible();
		expect((await row.boundingBox())?.height).toBe(28);
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
	});
});

test.describe("unapplied commit rows", () => {
	test.use({ scenario: "project-with-remote-branches.sh" });

	test("shows a single line and keeps the menu out of the label's space", async ({ appWindow }) => {
		await appWindow
			.getByRole("group", { name: "Pages" })
			.getByRole("button", { name: "Branches", exact: true })
			.click();
		const branch = appWindow.getByRole("treeitem", { name: "branch1", exact: true });
		await branch.getByRole("button", { name: "Unfold commits" }).click();
		const row = branch.getByRole("treeitem", { name: "branch1: first commit", exact: true });
		expect((await row.boundingBox())?.height).toBe(28);
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

test.describe("upstream history commit rows", () => {
	test.use({ scenario: "project-in-single-branch-three-branch-stack.sh" });
	for (const section of ["upstream", "history"] as const) {
		test(`shows PR labels in ${section}`, async ({ appWindow, electronApp }) => {
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
						inWorkspace: section === "history",
						commit: {
							...target.commit,
							id: section === "upstream" ? "a".repeat(40) : target.commit.id,
							committedAt: Date.now(),
						},
						review: {
							number: 42,
							title: "Improve parser diagnostics",
							htmlUrl: "https://example.com/pull/42",
							unitSymbol: "#",
							sourceBranch: "parser-diagnostics",
							labels: [{ name: "@gitbutler/lite", color: "5319e7", description: "Lite changes" }],
						},
					},
					...listing.commits.slice(section === "upstream" ? 0 : 1),
				],
			};
			await electronApp.evaluate(({ ipcMain }, data) => {
				ipcMain.removeHandler("workspaceTargetCommits");
				ipcMain.handle("workspaceTargetCommits", (_event, { from }: { from: string | null }) =>
					from === null ? data : { commits: [], hasMore: false },
				);
			}, enriched);
			await appWindow.reload();
			await appWindow
				.getByRole("button", {
					name: section === "upstream" ? "Unfold incoming commits" : "Unfold history",
					exact: true,
				})
				.click();
			const row = appWindow.getByRole("treeitem", {
				name: "Improve parser diagnostics",
				exact: true,
			});
			const label = row.getByText("@gitbutler/lite", { exact: true });
			await expect(label).toBeVisible();
			const bounds = await label.evaluate((element) => {
				const text = document.createRange();
				text.selectNodeContents(element);
				return {
					text: text.getBoundingClientRect().bottom,
					clip: element.getBoundingClientRect().bottom,
				};
			});
			expect(bounds.text).toBeLessThanOrEqual(bounds.clip);
			await expect(row).toHaveAccessibleDescription(/@gitbutler\/lite/);
		});
	}
});
