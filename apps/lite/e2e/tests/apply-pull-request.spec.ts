import type { ApplyOutcome, ForgeInfo, ForgeReview } from "@gitbutler/but-sdk";
import type { ElectronApplication, Page } from "@playwright/test";
import type { LiteElectronApi } from "../../electron/src/ipc.ts";
import { expect, test } from "../test.ts";

test.use({ scenario: "project-with-remote-branches.sh" });
test.slow();

type ReviewCall = { endpoint: string; params: unknown };

const forgeInfo: ForgeInfo = {
	name: "github",
	baseUrl: "https://github.com/acme/repo",
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
};

const review = {
	number: 42,
	title: "Fork PR",
	sourceBranch: "branch1",
	unitSymbol: "#",
} as ForgeReview;

const outcome: ApplyOutcome = {
	status: "applied",
	workspaceChanged: true,
	appliedBranches: [{ full: "refs/heads/branch1" }],
	workspaceRefCreated: false,
	conflictingStacks: [],
};

const mockForge = async (
	electronApp: ElectronApplication,
	appWindow: Page,
	accounts: Array<unknown>,
) => {
	await electronApp.evaluate(
		({ ipcMain }, { forgeInfo, accounts, review, outcome }) => {
			const calls: Array<ReviewCall> = [];
			(globalThis as { reviewCalls?: Array<ReviewCall> }).reviewCalls = calls;
			const responses: Record<string, unknown> = {
				forgeInfo,
				listKnownGithubAccounts: accounts,
				listReviews: [],
				getReview: review,
				reviewApply: outcome,
			};
			for (const [endpoint, response] of Object.entries(responses)) {
				ipcMain.removeHandler(endpoint);
				ipcMain.handle(endpoint, (_event, params) => {
					if (endpoint === "getReview" || endpoint === "reviewApply")
						calls.push({ endpoint, params });
					return response;
				});
			}
		},
		{ forgeInfo, accounts, review, outcome },
	);
	await appWindow.reload();
	await appWindow
		.getByRole("group", { name: "Pages" })
		.getByRole("button", { name: "Branches", exact: true })
		.click();
	await expect(appWindow.getByRole("button", { name: "New branch" })).toBeVisible();
};

const reviewCalls = (electronApp: ElectronApplication) =>
	electronApp.evaluate(() => (globalThis as { reviewCalls?: Array<ReviewCall> }).reviewCalls ?? []);

test("applies a pull request by number and follows its branch", async ({
	appWindow,
	electronApp,
}) => {
	await mockForge(electronApp, appWindow, [{ type: "patUsername", info: { username: "test" } }]);

	await appWindow.getByRole("button", { name: "Apply pull request" }).click();
	const dialog = appWindow.getByRole("dialog", { name: "Apply pull request" });
	const number = dialog.getByRole("textbox", { name: "Pull request number" });
	await expect(number).toBeFocused();
	await number.press("ArrowLeft");
	await expect(number).toBeFocused();

	await number.fill("#42");
	await number.press("Enter");
	await expect(dialog.getByText("Enter the number only, like 42.")).toBeVisible();
	expect(await reviewCalls(electronApp)).toEqual([]);

	await number.fill(" 42 ");
	await dialog.getByRole("button", { name: "Continue" }).click();
	await expect(dialog.getByText("Fork PR")).toBeVisible();

	// Fetching the fork is the backend's part; apply the seeded branch it would bring in.
	await appWindow.evaluate(async () => {
		const lite = (window as unknown as { lite: LiteElectronApi }).lite;
		const projectId = location.pathname.split("/")[2];
		if (projectId === undefined) throw new Error("No project in the URL");
		await lite.apply({ projectId, existingBranch: "refs/remotes/origin/branch1" });
	});
	await dialog.getByRole("button", { name: "Apply to workspace" }).click();

	await expect(dialog).toBeHidden();
	await expect(appWindow.getByRole("treeitem", { name: "branch1", exact: true })).toHaveAttribute(
		"aria-selected",
		"true",
	);
	const projectId = await appWindow.evaluate(() => location.pathname.split("/")[2]);
	// Refreshing the reviews after applying may fetch the previewed one again.
	const calls = await reviewCalls(electronApp);
	expect(calls.slice(0, 2)).toEqual([
		{ endpoint: "getReview", params: { projectId, reviewId: 42 } },
		{ endpoint: "reviewApply", params: { projectId, reviewId: 42 } },
	]);
	expect(calls.filter(({ endpoint }) => endpoint === "reviewApply")).toHaveLength(1);
});

test("is not offered without a GitHub account", async ({ appWindow, electronApp }) => {
	await mockForge(electronApp, appWindow, []);
	await expect(appWindow.getByRole("button", { name: "Apply pull request" })).toHaveCount(0);
});
