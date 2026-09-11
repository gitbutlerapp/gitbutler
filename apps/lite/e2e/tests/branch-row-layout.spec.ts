import type { CiCheck, ForgeInfo, ForgeReview } from "@gitbutler/but-sdk";
import { divergeBranch1 } from "../diverge.ts";
import { expect, test } from "../test.ts";

test.use({ scenario: "project-with-remote-branches.sh" });

test("keeps diverged branch actions inside a narrow row with PR checks", async ({
	appWindow,
	electronApp,
	testEnvironment,
}, testInfo) => {
	divergeBranch1(testEnvironment);
	const review: ForgeReview = {
		number: 15857,
		title: "Improve keyboard navigation in large repositories",
		sourceBranch: "branch1",
		targetBranch: "master",
		htmlUrl: "https://example.com/repo/pull/15857",
		body: null,
		author: null,
		labels: [
			{ name: "accessibility", color: "0e8a16", description: "Keyboard and screen reader support" },
			{ name: "@gitbutler/lite", color: "5319e7", description: null },
		],
		draft: false,
		sha: "0000000000000000000000000000000000000001",
		integrationCommitShas: [],
		createdAt: "2026-08-01T00:00:00Z",
		modifiedAt: "2026-08-02T00:00:00Z",
		mergedAt: null,
		closedAt: null,
		repositorySshUrl: null,
		repositoryHttpsUrl: null,
		repoOwner: null,
		headRepoIsFork: false,
		reviewers: [],
		autoMergeEnabled: false,
		unitSymbol: "#",
		lastSyncAt: "2026-08-02T00:00:00Z",
	};
	await electronApp.evaluate(({ ipcMain }, review) => {
		ipcMain.removeHandler("forgeInfo");
		ipcMain.handle(
			"forgeInfo",
			() =>
				({
					name: "github",
					baseUrl: "https://example.com/repo",
					commitUrlPath: "/commit/",
					prUrlPath: "/pull/",
					unit: { symbol: "#", name: "pull request", abbr: "PR" },
					posthogLabel: "",
					capabilities: {
						prService: true,
						checks: true,
						repoInfo: false,
						listService: false,
						reviewComments: false,
						reviewManagement: false,
					},
				}) satisfies ForgeInfo,
		);
		ipcMain.removeHandler("listCiChecks");
		ipcMain.handle(
			"listCiChecks",
			() =>
				[
					{
						id: 1,
						name: "Tests",
						status: { complete: { conclusion: "success", completed_at: null } },
						output: { title: "Tests", summary: "Passed", text: "" },
						startedAt: null,
						headSha: review.sha,
						url: "https://example.com/checks/1",
						htmlUrl: "https://example.com/checks/1",
						detailsUrl: "https://example.com/checks/1",
						pullRequests: [],
						reference: "branch1",
						lastSyncAt: review.lastSyncAt,
					},
				] satisfies Array<CiCheck>,
		);
		ipcMain.removeHandler("listReviews");
		ipcMain.handle("listReviews", () => [review]);
	}, review);

	await appWindow.reload();
	const branch = appWindow.getByRole("treeitem", { name: "branch1", exact: true });
	await expect(branch.getByText("PR #15857", { exact: true })).toBeVisible();
	await expect(branch.getByLabel("CI checks succeeded")).toBeVisible();
	const sidebar = appWindow.locator("#sidebar-panel");
	const handle = appWindow.locator('[role="separator"][aria-controls="sidebar-panel"]');
	const integrate = branch.getByRole("button", { name: "Integrate origin/branch1 into branch1" });
	await appWindow.setViewportSize({ width: 1400, height: 900 });
	for (const width of [260, 330, 420, 600]) {
		const bounds = await handle.boundingBox();
		const panel = await sidebar.boundingBox();
		if (!bounds || !panel) throw new Error("No sidebar resize handle");
		const x = bounds.x + bounds.width / 2;
		const y = bounds.y + bounds.height / 2;
		await appWindow.mouse.move(x, y);
		await appWindow.mouse.down();
		await appWindow.mouse.move(x + width - panel.width, y, { steps: 10 });
		await appWindow.mouse.up();
		await expect.poll(async () => (await sidebar.boundingBox())?.width).toBeCloseTo(width, 0);
		for (const fold of ["Fold commits", "Unfold commits"]) {
			await appWindow.mouse.move(0, 0);
			await expect(integrate).toBeVisible();
			await expect
				.poll(() =>
					integrate.evaluate((button) => {
						const meta = button.parentElement;
						const row = button.closest('[role="treeitem"]');
						if (!meta || !row) throw new Error("Integrate is outside a branch row");
						const bounds = meta.getBoundingClientRect();
						return Math.max(
							...Array.from(
								meta.children,
								(child) => child.getBoundingClientRect().right - bounds.right,
							),
							bounds.bottom - row.getBoundingClientRect().bottom,
						);
					}),
				)
				.toBeLessThanOrEqual(0.5);
			await integrate.click({ trial: true });
			if (fold === "Fold commits" && (width === 260 || width === 600)) {
				await appWindow.mouse.move(0, 0);
				await sidebar.screenshot({ path: testInfo.outputPath(`diverged-branch-${width}.png`) });
			}
			await branch.getByRole("button", { name: fold, exact: true }).click();
		}
	}
	await integrate.click();
	await expect(
		appWindow.getByRole("heading", { name: "Update branch1 from remote" }),
	).toBeVisible();
});
