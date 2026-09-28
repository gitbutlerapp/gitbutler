import type { ForgeInfo, ForgeReview } from "@gitbutler/but-sdk";
import { expect, test as base } from "../test.ts";

const test = base.extend<{ reviews: Array<ForgeReview> }>({
	reviews: [[], { option: true }],
});
test.use({ scenario: "project-in-single-branch-with-worktree.sh" });

const review: ForgeReview = {
	number: 2,
	title: "Teach the worktree to fly",
	sourceBranch: "W",
	targetBranch: "master",
	htmlUrl: "https://github.com/example/repo/pull/2",
	body: null,
	author: null,
	labels: [],
	draft: false,
	sha: "0000000000000000000000000000000000000002",
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

test.beforeEach(async ({ appWindow, electronApp, reviews }) => {
	await electronApp.evaluate(
		({ ipcMain }, { reviews }) => {
			const responses = {
				forgeInfo: {
					name: "github",
					baseUrl: "https://github.com/example/repo",
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
				} satisfies ForgeInfo,
				listReviews: reviews,
				listKnownGithubAccounts: [{ type: "patUsername", info: { username: "test" } }],
			};
			for (const [key, value] of Object.entries(responses)) {
				ipcMain.removeHandler(key);
				ipcMain.handle(key, () => value);
			}
		},
		{ reviews },
	);
	await appWindow.reload();
});

test("a worktree branch opens a pull request", async ({ appWindow }) => {
	await appWindow.getByRole("treeitem", { name: "W", exact: true }).click();
	await appWindow.getByRole("button", { name: "Create pull request", exact: true }).click();
	await expect(appWindow.getByPlaceholder("PR title")).toBeVisible();
});

test.describe("with an open pull request", () => {
	test.use({ reviews: [review] });
	test("a worktree branch row shows its title", async ({ appWindow }) => {
		await expect(
			appWindow.getByRole("treeitem", { name: "W", exact: true }).getByTitle(review.title),
		).toBeVisible();
	});
});
