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
	await appWindow.evaluate(() => {
		const projectId = location.pathname.split("/")[2];
		if (projectId === undefined) throw new Error("No project in the URL");
		const key = `pr_activity_inbox:v1:${projectId}`;
		const entries = JSON.parse(localStorage.getItem(key) ?? "[]") as Array<{
			id: string;
			author: string | null;
			snippet: string | null;
		}>;
		const first = entries[0];
		if (first === undefined) throw new Error("No seeded notification");
		entries.push({
			...first,
			id: "bot-1",
			author: "copilot-pull-request-reviewer",
			snippet: "Bot review",
		});
		localStorage.setItem(key, JSON.stringify(entries));
	});
	await appWindow.reload();
	await appWindow.getByRole("button", { name: "Notifications, 2 unread" }).click();
	const humanTab = appWindow.getByRole("tab", { name: "Humans (1)", exact: true });
	const agentTab = appWindow.getByRole("tab", { name: "Agents (1)", exact: true });
	const humanPanel = appWindow.getByRole("tabpanel", { name: "Humans (1)", exact: true });
	const agentPanel = appWindow.getByRole("tabpanel", { name: "Agents (1)", exact: true });
	await expect(humanTab).toHaveAttribute("aria-selected", "true");
	await expect(humanPanel).toContainText("Please take a look");
	await expect(humanPanel).not.toContainText("Bot review");
	// Keep the outgoing panel mounted long enough to inspect a tab transition.
	const transitionStyle = await appWindow.addStyleTag({
		content: `
		@keyframes hold-panel { from { opacity: 1; } to { opacity: 0.99; } }
		[role="tabpanel"][data-ending-style] { animation: hold-panel 60s linear; }
	`,
	});
	await humanTab.focus();
	await appWindow.keyboard.press("ArrowRight");
	await expect(agentTab).toBeFocused();
	await agentTab.click();
	await expect(agentPanel).toContainText("Bot review");
	await expect(agentPanel).not.toContainText("Please take a look");
	await expect(humanPanel).toContainText("Please take a look");
	await expect(humanPanel).not.toContainText("Bot review");
	await transitionStyle.evaluate((element) => {
		element.parentNode?.removeChild(element);
	});
	await appWindow.getByRole("button", { name: "Mark all read", exact: true }).click();
	await expect(appWindow.getByRole("tab", { name: "Agents", exact: true })).toBeVisible();
	await expect(humanTab).toBeVisible();
	await expect(appWindow.getByRole("button", { name: "Notifications, 1 unread" })).toBeVisible();
	await humanTab.click();
	await expect(humanPanel).toContainText("Please take a look");
});
