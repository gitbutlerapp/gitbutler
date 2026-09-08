import type { LiteElectronApi } from "../../electron/src/ipc.ts";
import type { ForgeInfo, ForgeReview } from "@gitbutler/but-sdk";
import { expect, test } from "../test.ts";

test.use({ scenario: "project-in-single-branch-three-branch-stack.sh" });

test("keeps unread PR activity off the Workspace tab", async ({ appWindow, electronApp }) => {
	const settings = await appWindow.evaluate(() =>
		(window as unknown as { lite: LiteElectronApi }).lite.readGUISettings(),
	);
	const review: ForgeReview = {
		number: 1,
		title: "Update C",
		sourceBranch: "C",
		targetBranch: "master",
		htmlUrl: "https://example.com/repo/pull/1",
		body: null,
		author: null,
		labels: [],
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
	await electronApp.evaluate(
		({ ipcMain }, { settings, review }) => {
			ipcMain.removeHandler("readGUISettings");
			ipcMain.handle("readGUISettings", () => ({ ...settings, prNotifications: "loud" }));
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
							checks: false,
							repoInfo: false,
							listService: false,
							reviewComments: false,
							reviewManagement: false,
						},
					}) satisfies ForgeInfo,
			);
			ipcMain.removeHandler("listReviews");
			ipcMain.handle("listReviews", () => [review]);
		},
		{ settings, review },
	);
	await appWindow.evaluate((review) => {
		const projectId = location.pathname.split("/")[2];
		if (projectId === undefined) throw new Error("No project in the URL");
		localStorage.setItem(
			`pr_activity_seen:v1:${projectId}`,
			JSON.stringify({ [review.number]: review.createdAt }),
		);
		localStorage.setItem(
			`pr_activity_inbox:v1:${projectId}`,
			JSON.stringify([
				{
					id: "comment-1",
					kind: "comment",
					review: review.number,
					reviewTitle: review.title,
					unitSymbol: review.unitSymbol,
					sourceBranch: review.sourceBranch,
					htmlUrl: review.htmlUrl,
					author: null,
					count: 1,
					snippet: "Please take a look",
					at: review.modifiedAt,
					seen: false,
				},
			]),
		);
	}, review);
	await appWindow.reload();

	await expect(appWindow.getByRole("button", { name: "Notifications, 1 unread" })).toBeVisible();
	const pages = appWindow.getByRole("group", { name: "Pages" });
	await expect(pages.getByRole("button", { name: "Workspace", exact: true })).toHaveText(
		"Workspace",
	);
});
